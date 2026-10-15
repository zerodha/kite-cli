use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::MarginsCommands;
use crate::util::read_json_array;
use anyhow::Result;
use kiteconnect::Request;

pub async fn handle(rt: &Runtime, command: MarginsCommands) -> Result<Output> {
    let request = match command {
        MarginsCommands::Orders(args) => {
            let mut query = Vec::new();
            if args.compact {
                query.push(("mode", "compact"));
            }
            Request::post("/margins/orders")
                .query(query)
                .json(read_json_array(&args.input)?)
        }
        MarginsCommands::Basket(args) => {
            let mut query = Vec::new();
            if args.consider_positions {
                query.push(("consider_positions", "true"));
            }
            if args.compact {
                query.push(("mode", "compact"));
            }
            Request::post("/margins/basket")
                .query(query)
                .json(read_json_array(&args.input)?)
        }
        MarginsCommands::Charges(args) => {
            Request::post("/charges/orders").json(read_json_array(&args.input)?)
        }
    };
    let value = rt
        .with_session(|api| api.call(request.clone()).raw())
        .await?;
    Ok(Output::view(value, render::amounts))
}
