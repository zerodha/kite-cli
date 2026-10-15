use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::HistoricalCommands;
use anyhow::Result;

pub async fn handle(rt: &Runtime, command: HistoricalCommands) -> Result<Output> {
    let value = match command {
        HistoricalCommands::Candles(args) => {
            let request = kiteconnect::Request::get(format!(
                "/instruments/historical/{}/{}",
                args.instrument_token, args.interval
            ))
            .query([
                ("from", args.from.as_str()),
                ("to", args.to.as_str()),
                ("continuous", if args.continuous { "1" } else { "0" }),
                ("oi", if args.oi { "1" } else { "0" }),
            ]);
            rt.with_session(|api| api.call(request.clone()).raw())
                .await?
        }
    };
    Ok(Output::view(value, render::generic))
}
