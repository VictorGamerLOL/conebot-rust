pub mod edit;
pub mod edit_list;
pub mod list;

use anyhow::{ anyhow, Result };
use serenity::{
    builder::CreateApplicationCommandOption,
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
        "list" => list::run(cmd_options, command, http).await?,
        "edit" => edit::run(cmd_options, command, http).await?,
        "edit_list" => edit_list::run(cmd_options, command, http).await?,
        &_ => anyhow::bail!("Unknown currency config subcommand."),
    }
    Ok(())
}

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("config")
        .kind(CommandOptionType::SubCommandGroup)
        .description("Configure various things about currencies or view them.")
        .add_sub_option(list::option())
        .add_sub_option(edit::option())
        .add_sub_option(edit_list::option());
    option
}
