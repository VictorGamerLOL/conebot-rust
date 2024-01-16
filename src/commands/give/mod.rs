use anyhow::{ anyhow, Result };
use serenity::{
    all::CommandInteraction,
    builder::CreateCommand,
    http::{ CacheHttp, Http },
    model::Permissions,
};

use crate::event_handler::command_handler::CommandOptions;

pub mod currency;
pub mod item;

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let (cmd_name, cmd_options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;

    match cmd_name.as_str() {
        "currency" => currency::run(cmd_options, command, http).await?,
        "item" => item::run(cmd_options, command, http).await?,
        &_ => anyhow::bail!("Unknown config subcommand."),
    }
    Ok(())
}

pub fn command() -> CreateCommand {
    CreateCommand::new("give")
        .description("Give a member something.")
        .dm_permission(false)
        .default_member_permissions(Permissions::MANAGE_GUILD)
        .add_option(currency::option())
        .add_option(item::option())
}
