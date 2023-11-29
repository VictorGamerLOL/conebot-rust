use anyhow::Result;
use serenity::{
    all::CommandInteraction,
    builder::{ CreateCommand, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::event_handler::command_handler::CommandOptions;

/// # Errors
/// Serenity stuff.
pub async fn run(
    _options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Send + Sync + CacheHttp
) -> Result<()> {
    let future = command.edit_response(
        &http,
        EditInteractionResponse::new().content("pong!")
    ).await?;
    Ok(())
}

pub fn application_command() -> CreateCommand {
    CreateCommand::new("ping").description("Pong!")
}
