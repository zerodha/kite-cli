use super::send_unless_dry_run;
use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::{AlertWriteArgs, AlertsCommands};
use crate::util::wire_value;
use anyhow::{Context, Result, bail};
use kiteconnect::{Alert, AlertParams, AlertRhsType, AlertStatus, AlertType, Request};
use std::collections::HashMap;
use std::future::IntoFuture;

pub async fn handle(rt: &Runtime, command: AlertsCommands) -> Result<Output> {
    let enabling = matches!(&command, AlertsCommands::Enable { .. });
    let value = match command {
        AlertsCommands::List { status } => {
            let filters: HashMap<_, _> = status
                .map(|status| ("status".into(), status))
                .into_iter()
                .collect();
            rt.with_session(|api| api.alerts(&filters).raw()).await?
        }
        AlertsCommands::Get { uuid } => rt.with_session(|api| api.alert(&uuid).raw()).await?,
        AlertsCommands::History { uuid } => {
            let value = rt
                .with_session(|api| api.alert_history(&uuid).raw())
                .await?;
            return Ok(Output::view(value, |value, doc| {
                render::records(value, &["created_at", "condition", "type", "uuid"], doc);
            }));
        }
        AlertsCommands::Create(args) => {
            let params = params(rt, &args, None).await?;
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.create_alert(params.clone()),
                "create the alert",
                "Alert submitted",
            )
            .await;
        }
        AlertsCommands::Modify { uuid, write } => {
            let current = rt
                .with_session(|api| api.alert(&uuid).into_future())
                .await?;
            let params = params(rt, &write, Some(&current)).await?;
            return send_unless_dry_run(
                rt,
                &write.confirm,
                |api| api.modify_alert(&uuid, params.clone()),
                "modify the alert",
                "Alert modification submitted",
            )
            .await;
        }
        AlertsCommands::Delete { uuid, confirm } => {
            let request =
                Request::delete("/alerts").query(uuid.iter().map(|uuid| ("uuid", uuid.as_str())));
            return send_unless_dry_run(
                rt,
                &confirm,
                |api| api.call(request.clone()),
                "delete the alerts",
                "Alert deletion submitted",
            )
            .await;
        }
        AlertsCommands::Enable { uuid, confirm } | AlertsCommands::Disable { uuid, confirm } => {
            let status = if enabling {
                AlertStatus::Enabled
            } else {
                AlertStatus::Disabled
            };
            let current = rt
                .with_session(|api| api.alert(&uuid).into_future())
                .await?;
            validate_current(&current)?;
            let request = crate::tui::data::alert_status_request(&uuid, status, current.alert_type);
            return send_unless_dry_run(
                rt,
                &confirm,
                |api| api.call(request.clone()),
                "change the alert status",
                "Alert status change submitted",
            )
            .await;
        }
    };
    Ok(Output::view(value, render::alerts))
}

fn validate_current(alert: &Alert) -> Result<()> {
    if !matches!(alert.alert_type, AlertType::Simple | AlertType::Ato) {
        bail!("unsupported alert type: {}", alert.alert_type);
    }
    if !matches!(alert.status, AlertStatus::Enabled | AlertStatus::Disabled) {
        bail!("cannot modify an alert with status {}", alert.status);
    }
    Ok(())
}

async fn params(
    rt: &Runtime,
    args: &AlertWriteArgs,
    current: Option<&Alert>,
) -> Result<AlertParams> {
    if let Some(alert) = current {
        validate_current(alert)?;
        if alert.alert_type != AlertType::Simple {
            bail!("use the TUI to edit ATO basket alerts");
        }
    }
    let name = args
        .name
        .as_deref()
        .or_else(|| current.map(|alert| alert.name.as_str()))
        .context("--name is required")?;
    if name.trim().is_empty() || name.chars().count() > 100 {
        bail!("alert name must contain 1 to 100 characters");
    }
    let (exchange, symbol) = if let Some(lhs) = &args.lhs {
        resolve_instrument(rt, lhs).await?
    } else {
        let alert = current.context("--lhs is required")?;
        (alert.lhs_exchange.clone(), alert.lhs_tradingsymbol.clone())
    };
    let attribute = args
        .attribute
        .as_deref()
        .or_else(|| current.map(|alert| alert.lhs_attribute.as_str()))
        .unwrap_or("LastTradedPrice");
    let operator = args
        .operator
        .as_deref()
        .or_else(|| current.map(|alert| alert.operator.as_str()))
        .context("--operator is required")?;
    let mut params = AlertParams::new(
        name,
        AlertType::Simple,
        exchange,
        symbol,
        attribute,
        wire_value(operator).context("unsupported alert operator")?,
        AlertRhsType::Constant,
    );
    params.status = current.map(|alert| alert.status);
    if let Some(value) = args.value {
        return Ok(params.rhs_constant(value));
    }
    if let Some(rhs) = &args.rhs {
        let (exchange, symbol) = resolve_instrument(rt, rhs).await?;
        return Ok(params.rhs_instrument(
            exchange,
            symbol,
            args.rhs_attribute
                .as_deref()
                .or_else(|| current.and_then(|alert| alert.rhs_attribute.as_deref()))
                .unwrap_or("LastTradedPrice"),
        ));
    }
    let alert = current.context("one of --value or --rhs is required")?;
    match alert.rhs_type.as_str() {
        "constant" => {
            if args.rhs_attribute.is_some() {
                bail!("--rhs-attribute requires an instrument condition; pass --rhs");
            }
            Ok(params.rhs_constant(alert.rhs_constant))
        }
        "instrument" => Ok(params.rhs_instrument(
            alert
                .rhs_exchange
                .as_deref()
                .context("stored alert has no right-hand exchange")?,
            alert
                .rhs_tradingsymbol
                .as_deref()
                .context("stored alert has no right-hand symbol")?,
            args.rhs_attribute
                .as_deref()
                .or(alert.rhs_attribute.as_deref())
                .context("stored alert has no right-hand attribute")?,
        )),
        other => bail!("unsupported right-hand condition type: {other}"),
    }
}

async fn resolve_instrument(rt: &Runtime, requested: &str) -> Result<(String, String)> {
    let (exchange, symbol) = requested
        .split_once(':')
        .filter(|(exchange, symbol)| !exchange.trim().is_empty() && !symbol.trim().is_empty())
        .with_context(|| format!("expected EXCHANGE:SYMBOL, got `{requested}`"))?;
    let store = crate::instruments::load(&rt.app_uuid()?)
        .await
        .map_err(anyhow::Error::msg)?;
    let instrument = store
        .by_exact_symbol(symbol)
        .into_iter()
        .find(|instrument| instrument.exchange.eq_ignore_ascii_case(exchange.trim()))
        .with_context(|| format!("instrument not found: {requested}"))?;
    let exchange = if instrument.is_index() {
        "INDICES"
    } else {
        &instrument.exchange
    };
    Ok((exchange.into(), instrument.tradingsymbol.clone()))
}
