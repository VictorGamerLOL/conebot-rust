use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption },
    http::{ CacheHttp, Http },
};

use crate::event_handler::command_handler::CommandOptions;

pub mod create_entry;
pub mod delete_entry;
pub mod edit_entry;

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Send + Sync
) -> Result<()> {
    let (subcommand, command_options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("No subcommand provided."))?;
    match subcommand.as_str() {
        "create_entry" => create_entry::run(command_options, command, http).await?,
        "delete_entry" => delete_entry::run(command_options, command, http).await?,
        _ => {
            return Err(anyhow!("Invalid subcommand provided."));
        }
    }
    Ok(())
}

pub fn command() -> CreateCommand {
    CreateCommand::new("config_store")
        .description("Configure the store for your server.")
        .dm_permission(false)
        .add_option(create_entry::option())
        .add_option(delete_entry::option())
        .add_option(edit_entry::option())
}
