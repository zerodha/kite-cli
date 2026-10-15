use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::UserCommands;
use anyhow::Result;

pub async fn handle(rt: &Runtime, command: UserCommands) -> Result<Output> {
    let renderer = match &command {
        UserCommands::Profile => None,
        UserCommands::Margins { segment } => Some(segment.clone()),
    };
    let value = match command {
        UserCommands::Profile => rt.with_session(|api| api.profile().raw()).await?,
        UserCommands::Margins { segment } => match segment.as_deref() {
            Some(segment) => {
                rt.with_session(|api| api.segment_margins(segment).raw())
                    .await?
            }
            None => rt.with_session(|api| api.margins().raw()).await?,
        },
    };
    Ok(Output::view(value, move |value, doc| match renderer {
        None => render::profile(value, doc),
        Some(segment) => render::margins(value, segment.as_deref(), doc),
    }))
}
