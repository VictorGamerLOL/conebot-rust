pub mod exchange;

use anyhow::{ anyhow, Result };
use serenity::{ all::CommandInteraction, builder::CreateCommand, http::{ CacheHttp, Http } };

use crate::event_handler::command_handler::CommandOptions;

/// # Errors
/// Serenity stuff.
pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
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

pub fn command() -> CreateCommand {
    CreateCommand::new("currency")
        .description("Commands related to managing currencies.")
        .dm_permission(false)
        .add_option(exchange::option())
}
