use crate::app::output::Output;
use crate::app::render::{self, Doc, Term};
use crate::app::runtime::{Runtime, SESSION_FIELDS};
use crate::app_api::trusted::TrustedAuth;
use crate::cli::AuthCommands;
use anyhow::{Context, Result, bail};
use kiteconnect::error::sanitize_error;
use serde_json::json;
use std::io::{self, IsTerminal, Write};

pub async fn handle(rt: &Runtime, command: AuthCommands) -> Result<Output> {
    match command {
        AuthCommands::Login { user_id } => {
            let creds = rt.resolve_credentials()?;
            creds.api_root()?;
            creds.api_key()?;
            if rt.json || !io::stdin().is_terminal() {
                bail!(
                    "interactive `auth login` cannot run under --json or without a TTY; run it in an interactive terminal"
                );
            }
            interactive_login(rt, user_id).await?;
            Ok(Output::None)
        }
        AuthCommands::Logout { .. } => logout(rt).await,
    }
}

async fn logout(rt: &Runtime) -> Result<Output> {
    let (_, _, config) = rt.load_config()?;
    let mut revoked = false;
    let had_session = config.access_token.is_some();
    let mut warning = None;
    if let Some(access_token) = config.access_token {
        let revoke = async {
            let creds = rt.resolve_credentials()?;
            TrustedAuth::new(creds.api_root()?, creds.api_key()?, &rt.app_uuid()?)?
                .invalidate(&access_token)
                .await
        };
        match revoke.await {
            Ok(()) => revoked = true,
            Err(err) => {
                let message = sanitize_error(&err.to_string());
                if rt.json {
                    eprintln!(
                        "warning: could not revoke session: {message}; the token may remain active until expiry. Sign in on Kite and log out there to revoke it"
                    );
                }
                warning = Some(message);
            }
        }
    }
    let path = rt.clear_session()?;
    Ok(Output::view(
        json!({
            "logged_out": true,
            "revoked": revoked,
            "cleared_config_fields": SESSION_FIELDS,
            "path": path,
        }),
        move |value, doc| {
            if let Some(message) = warning.as_deref() {
                doc.warn(&format!("could not revoke the session on Kite: {message}; the token may remain active until expiry. Sign in on Kite and log out there to revoke it"));
            }
            doc.success("Logged out");
            let path = render::s(value, "path");
            let detail = if revoked {
                format!("Session revoked on Kite and removed from {path}")
            } else if had_session {
                format!("Session removed from {path}; the token expires at 6 AM IST")
            } else {
                "No saved session; nothing to revoke.".into()
            };
            doc.line(vec![format!("  {detail}").into()]);
        },
    ))
}

async fn interactive_login(rt: &Runtime, user_id_arg: Option<String>) -> Result<()> {
    let creds = rt.resolve_credentials()?;
    let api_key = creds.api_key()?;
    let auth = TrustedAuth::new(creds.api_root()?, api_key, &rt.app_uuid()?)?;
    eprintln!("Log in to Kite");

    let user_id = match user_id_arg {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => {
            let saved = creds.user_id.as_deref().unwrap_or_default();
            let label = if saved.is_empty() {
                "User ID: ".to_string()
            } else {
                format!("User ID [{saved}]: ")
            };
            let entered = prompt(&label)?;
            if entered.is_empty() {
                saved.to_string()
            } else {
                entered
            }
        }
    }
    .to_uppercase();
    if user_id.is_empty() {
        bail!("user id cannot be empty");
    }
    let password = prompt_masked("Password: ")?;
    let challenge = {
        let _status = render::Status::start("Signing in…", rt.json);
        auth.login(&user_id, &password).await?
    };
    let twofa_type = if challenge.twofa_types.len() > 1 {
        eprintln!("Verify with: {}", challenge.twofa_types.join(", "));
        let selected = prompt(&format!("Method [{}]: ", challenge.twofa_type))?;
        if selected.is_empty() {
            challenge.twofa_type.clone()
        } else {
            challenge
                .twofa_types
                .iter()
                .find(|kind| kind.eq_ignore_ascii_case(&selected))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("choose one of the offered verification methods"))?
        }
    } else {
        challenge.twofa_type.clone()
    };
    if twofa_type.eq_ignore_ascii_case("sms") {
        auth.send_otp(&challenge).await?;
        eprintln!("Sent an OTP by SMS.");
    }
    let code = prompt(&format!("{} code: ", twofa_type.to_uppercase()))?;
    if code.is_empty() {
        bail!("verification code cannot be empty");
    }
    let session = {
        let _status = render::Status::start("Verifying…", rt.json);
        auth.twofa(&challenge, &twofa_type, &code).await?
    };
    let path = rt.save_session(&session)?;
    let display_name = session.user_name.as_deref().unwrap_or(&session.user_id);
    let mut doc = Doc::default();
    doc.success(&format!(
        "Logged in as {display_name} ({})",
        session.user_id
    ));
    doc.line(vec![
        format!("  Session saved to {}", path.display()).into(),
    ]);
    doc.write(
        &mut io::stdout().lock(),
        Term::stdout(),
        &mut io::stderr().lock(),
        Term::stderr(),
    )?;
    Ok(())
}

fn prompt(message: &str) -> Result<String> {
    render::prompt(message)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_masked(message: &str) -> Result<String> {
    loop {
        let value = read_masked(message)?;
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
        eprintln!("That cannot be empty. Please try again.");
    }
}

fn read_masked(message: &str) -> Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    render::prompt(message)?;
    enable_raw_mode().context("failed to enable raw mode for masked input")?;

    let mut input = String::new();
    let mut cancelled = false;
    let mut read_err: Option<io::Error> = None;

    loop {
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        cancelled = true;
                        break;
                    }
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) if input.is_empty() => {
                        cancelled = true;
                        break;
                    }
                    (KeyCode::Enter, _) => break,
                    (KeyCode::Backspace, _) if input.pop().is_some() => {
                        eprint!("\u{8} \u{8}");
                        let _ = io::stderr().flush();
                    }
                    (KeyCode::Char(c), modifiers)
                        if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        input.push(c);
                        eprint!("*");
                        let _ = io::stderr().flush();
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(err) => {
                read_err = Some(err);
                break;
            }
        }
    }

    let _ = disable_raw_mode();
    eprintln!();

    if let Some(err) = read_err {
        return Err(anyhow::Error::from(err).context("failed to read masked input"));
    }
    if cancelled {
        eprintln!("Login cancelled.");
        std::process::exit(130);
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::runtime::ConfigFile;
    use crate::cli::Cli;
    use clap::Parser;

    #[tokio::test]
    async fn logout_without_a_saved_token_reports_no_revocation() {
        let _env = crate::app::runtime::TestEnv::clear().await;
        let dir = std::env::temp_dir().join(format!("kite-logout-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let cli = Cli::parse_from([
            "kite",
            "--config",
            path.to_str().unwrap(),
            "auth",
            "logout",
            "--confirm",
        ]);
        let rt = Runtime::from_cli(&cli);
        rt.save_config(&ConfigFile {
            api_key: Some("saved-key".into()),
            ..Default::default()
        })
        .unwrap();
        let Output::View(output, _) = logout(&rt).await.unwrap() else {
            panic!("expected JSON logout output");
        };
        assert_eq!(output["revoked"], false);
        assert_eq!(output["logged_out"], true);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
