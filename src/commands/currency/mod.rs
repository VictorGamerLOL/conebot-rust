pub mod exchange;

use anyhow::{ anyhow, Result };
use serenity::{
    builder::CreateApplicationCommand,
    http::{ CacheHttp, Http },
    model::prelude::application_command::{ ApplicationCommandInteraction, CommandDataOption },
};

use crate::event_handler::command_handler::CommandOptions;

/// # Errors
/// Serenity stuff.
pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let (cmd_name, options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("No subcommand found"))?;
    match cmd_name.as_str() {
        "exchange" => exchange::run(options, command, &http).await?,
        _ => {
            return Err(anyhow!("Unknown subcommand: {}", cmd_name));
        }
    }
    Ok(())
}

#[must_use]
pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command
        .name("currency")
        .description("Commands related to managing currencies.")
        .dm_permission(false)
        .add_option(exchange::option());
    command
}
