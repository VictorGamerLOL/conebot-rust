pub mod currency;
pub mod item;

use anyhow::{ anyhow, Result };
use serenity::{
    builder::{ CreateApplicationCommand, CreateApplicationCommandOption },
    http::{ CacheHttp, Http },
    model::prelude::{
        application_command::ApplicationCommandInteraction,
        command::CommandOptionType,
    },
};

use crate::event_handler::command_handler::CommandOptions;

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let (cmd_name, cmd_options): (String, CommandOptions) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;
    match cmd_name.as_str() {
        "currency" => currency::run(cmd_options, command, http).await?,
        "item" => item::run(cmd_options, command, http).await?,
        &_ => anyhow::bail!("Unknown config subcommand."),
    }
    Ok(())
}

pub fn application_command() -> CreateApplicationCommand {
    let mut option = CreateApplicationCommand::default();
    option
        .name("config")
        .description("Configure various things about currencies or view them.")
        .add_option(currency::option())
        .add_option(item::option())
        .dm_permission(false);
    option
}
