use anyhow::{ anyhow, bail, Result };
use serenity::{
    builder::CreateApplicationCommandOption,
    http::{ CacheHttp, Http },
    model::application::{
        command::CommandOptionType,
        interaction::application_command::ApplicationCommandInteraction,
    },
};

use crate::event_handler::command_handler::CommandOptions;

pub mod create;
pub mod delete;
pub mod edit;
pub mod edit_list;
pub mod list;

pub async fn run(
    options: CommandOptions,
    _command: &ApplicationCommandInteraction,
    _http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let (cmd_name, cmd_options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;
    match cmd_name.as_str() {
        "list" => list::run(cmd_options, _command, _http).await?,
        "edit" => edit::run(cmd_options, _command, _http).await?,
        "edit_list" => edit_list::run(cmd_options, _command, _http).await?,
        "create" => create::run(cmd_options, _command, _http).await?,
        "delete" => delete::run(cmd_options, _command, _http).await?,
        &_ => bail!("Unknown currency config subcommand."),
    }
    Ok(())
}
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("currency")
        .description("Configure various things about currencies or view them.")
        .kind(CommandOptionType::SubCommandGroup)
        .add_sub_option(list::option())
        .add_sub_option(edit::option())
        .add_sub_option(edit_list::option())
        .add_sub_option(create::option())
        .add_sub_option(delete::option());
    option
}
