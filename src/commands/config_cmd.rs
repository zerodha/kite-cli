use crate::app::errors::format_error;
use crate::app::output::{Output, redact};
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::ConfigCommands;
use anyhow::Result;
use kiteconnect::Request;
use serde_json::json;

pub async fn doctor(rt: &Runtime, no_live: bool) -> Result<Output> {
    let creds = rt.resolve_credentials()?;
    let has_session = creds.access_token.is_some();

    let mut setup = Vec::new();
    for setting in [creds.api_root(), creds.api_key()] {
        if let Err(error) = setting {
            setup.push(error.to_string());
        }
    }
    let configured = setup.is_empty();
    if configured && !has_session {
        setup.push("run `kite auth login`".to_string());
    }

    let (api, api_ok) = match (no_live, configured, has_session) {
        (true, _, _) => (json!({ "checked": false, "reason": "skipped" }), true),
        (false, false, _) => (
            json!({ "checked": false, "reason": "not configured" }),
            true,
        ),
        (false, true, false) => (json!({ "checked": false, "reason": "not logged in" }), true),
        (false, true, true) => match rt
            .with_session(|api| api.call(Request::get("/user/margins")).raw())
            .await
        {
            Ok(_) => (
                json!({ "checked": true, "ok": true, "endpoint": "/user/margins" }),
                true,
            ),
            Err(err) => {
                if err
                    .downcast_ref::<kiteconnect::Error>()
                    .is_some_and(kiteconnect::Error::is_session_expired)
                    || err.to_string() == "session expired; run `kite auth login`"
                {
                    setup.push("run `kite auth login`".to_string());
                }
                (
                    json!({ "checked": true, "ok": false, "endpoint": "/user/margins", "error": format_error(&err) }),
                    false,
                )
            }
        },
    };

    let passed = setup.is_empty() && api_ok;
    Ok(Output::check(
        passed,
        json!({
            "ok": passed,
            "version": env!("CARGO_PKG_VERSION"),
            "api_root": creds.api_root,
            "ticker_url": creds.ticker_url,
            "app_uuid": creds.app_uuid,
            "config": {
                "path": creds.path,
                "exists": creds.config_exists,
            },
            "auth": {
                "api_key": creds.api_key,
                "user_id": {
                    "present": creds.user_id.is_some(),
                    "value": redact(creds.user_id.as_deref(), false),
                },
                "access_token": {
                    "present": creds.access_token.is_some(),
                    "value": redact(creds.access_token.as_deref(), false),
                },
            },
            "api": api,
            "setup": setup,
            "env_vars": ["KITE_API_ROOT", "KITE_API_KEY", "KITE_TICKER_URL", "KITE_USER_ID", "KITE_ACCESS_TOKEN", "KITE_CONFIG"],
        }),
        render::doctor,
    ))
}

pub fn handle(rt: &Runtime, command: &ConfigCommands) -> Result<Output> {
    match command {
        ConfigCommands::Path => Ok(Output::view(
            json!({ "path": rt.config_path()? }),
            render::path,
        )),
        ConfigCommands::Show { reveal } => {
            let creds = rt.resolve_credentials()?;
            Ok(Output::view(
                json!({
                    "path": creds.path,
                    "exists": creds.config_exists,
                    "effective": {
                        "api_root": creds.api_root,
                        "ticker_url": creds.ticker_url,
                        "app_uuid": creds.app_uuid,
                        "user_id": redact(creds.user_id.as_deref(), *reveal),
                        "access_token": redact(creds.access_token.as_deref(), *reveal),
                        "api_key": creds.api_key,
                    }
                }),
                render::generic,
            ))
        }
    }
}
