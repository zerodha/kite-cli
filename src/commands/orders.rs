use super::send_unless_dry_run;
use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::OrdersCommands;
use crate::util::{order_modify_params, order_place_params, wire_value};
use anyhow::{Result, bail};
use serde_json::Value;

pub async fn handle(rt: &Runtime, command: OrdersCommands) -> Result<Output> {
    let (book, empty) = match &command {
        OrdersCommands::List {
            status,
            tradingsymbol,
        } => {
            let filters = status
                .iter()
                .chain(tradingsymbol.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            (
                true,
                if filters.is_empty() {
                    "No orders today.".into()
                } else {
                    format!("No {filters} orders today.")
                },
            )
        }
        OrdersCommands::Get { .. } => (true, "No order history.".into()),
        _ => (false, "No trades today.".into()),
    };
    let value = match command {
        OrdersCommands::List {
            status,
            tradingsymbol,
        } => {
            let data = rt.with_session(|api| api.orders().raw()).await?;
            filter_array(data, |item| {
                matches_string_field(item, "status", status.as_deref())
                    && matches_string_field(item, "tradingsymbol", tradingsymbol.as_deref())
            })
        }
        OrdersCommands::Get { order_id } => {
            rt.with_session(|api| api.order_history(&order_id).raw())
                .await?
        }
        OrdersCommands::Trades { order_id } => match order_id {
            Some(id) => rt.with_session(|api| api.order_trades(&id).raw()).await?,
            None => rt.with_session(|api| api.trades().raw()).await?,
        },
        OrdersCommands::Place(args) => {
            let variety = wire_value(&args.variety)?;
            let params = order_place_params(&args)?;
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.place_order(variety, params.clone()),
                "place the order",
                "Order submitted",
            )
            .await;
        }
        OrdersCommands::Modify(args) => {
            let Some(params) = order_modify_params(&args)? else {
                bail!("nothing to modify; pass at least one order parameter");
            };
            let variety = wire_value(&args.variety)?;
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.modify_order(variety, &args.order_id, params.clone()),
                "modify the order",
                "Order modification submitted",
            )
            .await;
        }
        OrdersCommands::Cancel(args) => {
            let variety = wire_value(&args.variety)?;
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.cancel_order(variety, &args.order_id, None),
                "cancel the order",
                "Order cancellation submitted",
            )
            .await;
        }
    };
    Ok(Output::view(value, move |value, doc| {
        if book {
            render::orders(value, &empty, doc);
        } else {
            trades(value, doc);
        }
    }))
}

fn trades(value: &Value, doc: &mut render::Doc) {
    render::records(
        value,
        &[
            "fill_timestamp",
            "trade_id",
            "order_id",
            "exchange",
            "tradingsymbol",
            "transaction_type",
            "product",
            "quantity",
            "average_price",
        ],
        doc,
    );
}

fn filter_array<F>(value: Value, mut predicate: F) -> Value
where
    F: FnMut(&Value) -> bool,
{
    match value {
        Value::Array(items) => {
            Value::Array(items.into_iter().filter(|item| predicate(item)).collect())
        }
        other => other,
    }
}

fn matches_string_field(item: &Value, field: &str, expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    item.get(field)
        .and_then(Value::as_str)
        .is_some_and(|value| {
            if field == "status" && expected.eq_ignore_ascii_case("OPEN") {
                matches!(
                    value.to_ascii_uppercase().as_str(),
                    "OPEN"
                        | "TRIGGER PENDING"
                        | "OPEN PENDING"
                        | "AMO REQ RECEIVED"
                        | "MODIFY PENDING"
                        | "VALIDATION PENDING"
                        | "PUT ORDER REQ RECEIVED"
                        | "MODIFY VALIDATION PENDING"
                )
            } else {
                value.eq_ignore_ascii_case(expected)
            }
        })
}
