mod alerts;
mod auth;
mod config_cmd;
mod gtt;
mod historical;
mod margins;
mod market;
mod mf;
mod orders;
mod portfolio;
mod request;
mod stream;
mod user;

use crate::app::output::Output;
use crate::app::runtime::Runtime;
use crate::cli::{Cli, Commands, ConfirmArgs, LegacyQuotesCommands, StreamMode, TuiArgs};
use anyhow::Result;
use kiteconnect::{Call, KiteConnect};
use std::io::{self, IsTerminal};

/// # Errors
/// Returns an error if command validation, configuration, or execution fails.
pub async fn run(cli: Cli) -> Result<Output> {
    let rt = Runtime::from_cli(&cli);

    let Some(command) = cli.command else {
        if rt.json || !io::stdin().is_terminal() {
            eprintln!("no command given; run `kite --help` or run interactively for setup");
            std::process::exit(2);
        }
        crate::tui::run(
            &rt,
            TuiArgs {
                mode: StreamMode::Quote,
                token: vec![],
                tokens: vec![],
            },
        )
        .await?;
        return Ok(Output::None);
    };

    match command {
        Commands::Doctor { no_live } => config_cmd::doctor(&rt, no_live).await,
        Commands::Config { command } => config_cmd::handle(&rt, &command),
        Commands::Auth { command } => auth::handle(&rt, command).await,
        Commands::User { command } => user::handle(&rt, command).await,
        Commands::Orders { command } => orders::handle(&rt, command).await,
        Commands::Portfolio { command } => portfolio::handle(&rt, command).await,
        Commands::Market { command } => market::handle(&rt, command).await,
        Commands::Quotes { command } => match command {
            LegacyQuotesCommands::Ltp(args) => {
                market::quote(&rt, market::QuoteKind::Ltp, args).await
            }
            LegacyQuotesCommands::Ohlc(args) => {
                market::quote(&rt, market::QuoteKind::Ohlc, args).await
            }
            LegacyQuotesCommands::Full(args) => {
                market::quote(&rt, market::QuoteKind::Full, args).await
            }
        },
        Commands::Historical { command } => historical::handle(&rt, command).await,
        Commands::Margins { command } => margins::handle(&rt, command).await,
        Commands::Gtt { command } => gtt::handle(&rt, command).await,
        Commands::Alerts { command } => alerts::handle(&rt, command).await,
        Commands::Mf { command } => mf::handle(&rt, command).await,
        Commands::Stream { command } => stream::handle(&rt, command).await,
        Commands::Tui(args) => stream::tui(&rt, args).await,
        Commands::Request { command } => request::handle(&rt, command).await,
    }
}

async fn send_unless_dry_run<T>(
    rt: &Runtime,
    confirm: &ConfirmArgs,
    build: impl Fn(KiteConnect) -> Call<T>,
    action: &str,
    headline: &'static str,
) -> Result<Output> {
    let call = build(KiteConnect::builder("dry-run").build()?);
    let request =
        crate::util::dry_run_request(call.request(), &format!("add --confirm to {action}"));
    let summary = crate::app::render::order_summary(&request["body"]["form"]);
    let form = request["body"]["form"].clone();
    let value = if confirm.is_dry_run() {
        request
    } else {
        rt.with_session(|api| build(api).raw())
            .await
            .map_err(|error| write_error(error, "check the account state with `kite orders list`, `kite gtt list`, `kite alerts list`, or `kite portfolio positions` before retrying"))?
    };
    Ok(Output::view(value, move |value, doc| {
        crate::app::render::write_result(value, headline, &summary, doc);
        if form.get("old_product").is_some() && value["dry_run"] != true {
            crate::app::render::generic(&form, doc);
        }
    }))
}

pub(super) fn write_error(error: anyhow::Error, check: &str) -> anyhow::Error {
    if matches!(
        error.downcast_ref::<kiteconnect::Error>(),
        Some(kiteconnect::Error::Transport(_))
    ) {
        error.context(format!("the write outcome is unknown; {check}"))
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[tokio::test]
    async fn every_dry_run_write_ignores_invalid_config() {
        let dir = std::env::temp_dir().join(format!("kite-command-dry-run-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{invalid json").unwrap();
        for (args, method, endpoint) in [
            (
                "orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY --order-type MARKET --quantity 1 --product CNC",
                "POST",
                "/orders/regular",
            ),
            (
                "orders modify --order-id 123 --quantity 2",
                "PUT",
                "/orders/regular/123",
            ),
            (
                "orders cancel --order-id 123",
                "DELETE",
                "/orders/regular/123",
            ),
            (
                "portfolio convert --exchange NSE --tradingsymbol INFY --transaction-type BUY --position-type day --quantity 1 --old-product MIS --new-product CNC",
                "PUT",
                "/portfolio/positions",
            ),
            (
                "gtt place --condition-json {} --orders-json []",
                "POST",
                "/gtt/triggers",
            ),
            (
                "gtt modify --trigger-id 123 --type single --condition-json {} --orders-json []",
                "PUT",
                "/gtt/triggers/123",
            ),
            ("gtt delete --trigger-id 123", "DELETE", "/gtt/triggers/123"),
            ("alerts delete alert-uuid", "DELETE", "/alerts"),
            ("request post /audit --form safe=value", "POST", "/audit"),
            ("request put /audit --body-json {}", "PUT", "/audit"),
            ("request delete /audit", "DELETE", "/audit"),
        ] {
            for flags in [vec![], vec!["--dry-run"], vec!["--confirm", "--dry-run"]] {
                let cli = Cli::parse_from(
                    ["kite", "--json", "--config", path.to_str().unwrap()]
                        .into_iter()
                        .chain(args.split_whitespace())
                        .chain(flags),
                );
                let Output::View(value, _) = run(cli).await.unwrap() else {
                    panic!("expected JSON");
                };
                assert_eq!(value["dry_run"], true);
                assert_eq!(value["method"], method);
                assert_eq!(value["path"], endpoint);
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
