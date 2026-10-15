use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::{MfCommands, MfInstrumentDumpArgs};
use crate::util::{csv_records, search_csv};
use anyhow::{Context, Result};
use kiteconnect::{KiteConnect, RawResponse};
use serde_json::json;
use std::fs;

const INSTRUMENT_COLUMNS: &[&str] = &[
    "tradingsymbol",
    "name",
    "minimum_purchase_amount",
    "minimum_redemption_quantity",
    "last_price",
];

pub async fn handle(rt: &Runtime, command: MfCommands) -> Result<Output> {
    let holdings = matches!(&command, MfCommands::Holdings);
    let columns: &[&str] = match &command {
        MfCommands::Holdings => &[
            "fund",
            "folio",
            "quantity",
            "average_price",
            "last_price",
            "pnl",
        ],
        MfCommands::Sips => &[
            "sip_id",
            "fund",
            "status",
            "instalment_amount",
            "frequency",
            "next_instalment",
            "completed_instalments",
        ],
        MfCommands::Orders | MfCommands::Order { .. } => &[
            "order_id",
            "fund",
            "transaction_type",
            "status",
            "quantity",
            "amount",
            "average_price",
            "last_price",
            "status_message",
        ],
        MfCommands::Instruments(_) | MfCommands::Search(_) => INSTRUMENT_COLUMNS,
    };
    let value = match command {
        MfCommands::Orders => rt.with_session(|api| api.mf_orders().raw()).await?,
        MfCommands::Order { order_id } => {
            rt.with_session(|api| api.mf_order_info(&order_id).raw())
                .await?
        }
        MfCommands::Sips => rt.with_session(|api| api.mf_sips().raw()).await?,
        MfCommands::Holdings => rt.with_session(|api| api.mf_holdings().raw()).await?,
        MfCommands::Instruments(args) => return instruments(rt, args).await,
        MfCommands::Search(args) => {
            let response = mf_instruments_csv(rt).await?;
            let csv = String::from_utf8_lossy(response.body());
            let matches = search_csv(&csv, &args.query, args.limit)?;
            json!({ "query": args.query, "matches": matches })
        }
    };
    Ok(Output::view(value, move |value, doc| {
        if holdings {
            let mut value = value.clone();
            if let Some(rows) = value.as_array_mut() {
                for row in rows {
                    if let Some(row) = row.as_object_mut() {
                        let pnl = row
                            .get("quantity")
                            .and_then(serde_json::Value::as_f64)
                            .zip(row.get("last_price").and_then(serde_json::Value::as_f64))
                            .zip(row.get("average_price").and_then(serde_json::Value::as_f64))
                            .map(|((quantity, last_price), average_price)| {
                                quantity * (last_price - average_price)
                            });
                        row.insert("pnl".into(), json!(pnl));
                    }
                }
            }
            render::records(&value, columns, doc);
        } else {
            render::records(value, columns, doc);
        }
    }))
}

async fn mf_instruments_csv(rt: &Runtime) -> Result<RawResponse> {
    Ok(KiteConnect::builder("unused")
        .user_agent(crate::app::http::user_agent())
        .header("X-Kite-App-UUID", rt.app_uuid()?)
        .build()?
        .mf_instruments()
        .response()
        .await?)
}

async fn instruments(rt: &Runtime, args: MfInstrumentDumpArgs) -> Result<Output> {
    let response = mf_instruments_csv(rt).await?;
    let csv = String::from_utf8_lossy(response.body());
    if let Some(out) = args.out {
        fs::write(&out, response.body())
            .with_context(|| format!("failed to write {}", out.display()))?;
        return Ok(Output::view(
            json!({
                "path": out,
                "endpoint": "/mf/instruments",
                "bytes": response.body().len(),
                "format": "csv",
            }),
            render::generic,
        ));
    }

    let (records, truncated) = csv_records(&csv, args.limit)?;
    Ok(Output::view(
        json!({ "endpoint": "/mf/instruments", "format": "csv", "limit": args.limit, "truncated": truncated, "records": records }),
        |value, doc| render::records(value, INSTRUMENT_COLUMNS, doc),
    ))
}
