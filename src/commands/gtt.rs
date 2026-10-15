use super::send_unless_dry_run;
use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::GttCommands;
use crate::util::gtt_payload;
use anyhow::{Context, Result};
use kiteconnect::Request;

pub async fn handle(rt: &Runtime, command: GttCommands) -> Result<Output> {
    let value = match command {
        GttCommands::List => rt.with_session(|api| api.gtts().raw()).await?,
        GttCommands::Get { trigger_id } => rt.with_session(|api| api.gtt(trigger_id).raw()).await?,
        GttCommands::Place(args) => {
            let (condition, orders) = gtt_payload(&args)?;
            let request = Request::post("/gtt/triggers").form([
                ("type", args.trigger_type.unwrap_or_else(|| "single".into())),
                ("condition", condition.to_string()),
                ("orders", orders.to_string()),
            ]);
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.call(request.clone()),
                "place the GTT trigger",
                "GTT submitted",
            )
            .await;
        }
        GttCommands::Modify(mut args) => {
            let trigger_type = if let Some(trigger_type) = &args.write.trigger_type {
                trigger_type.clone()
            } else {
                let current = rt
                    .with_session(|api| api.gtt(args.trigger_id).raw())
                    .await?;
                current["type"]
                    .as_str()
                    .context("stored GTT has no type")?
                    .to_owned()
            };
            args.write.trigger_type = Some(trigger_type.clone());
            let (condition, orders) = gtt_payload(&args.write)?;
            let request = Request::put(format!("/gtt/triggers/{}", args.trigger_id)).form([
                ("type", trigger_type),
                ("condition", condition.to_string()),
                ("orders", orders.to_string()),
            ]);
            return send_unless_dry_run(
                rt,
                &args.write.confirm,
                |api| api.call(request.clone()),
                "modify the GTT trigger",
                "GTT modification submitted",
            )
            .await;
        }
        GttCommands::Delete(args) => {
            return send_unless_dry_run(
                rt,
                &args.confirm,
                |api| api.delete_gtt(args.trigger_id),
                "delete the GTT trigger",
                "GTT deletion submitted",
            )
            .await;
        }
    };
    Ok(Output::view(value, render::gtt))
}
