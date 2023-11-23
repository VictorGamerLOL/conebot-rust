use anyhow::{ anyhow, bail, Result };
use serenity::{
    builder::CreateApplicationCommandOption,
    http::{ CacheHttp, Http },
    model::application::command::CommandOptionType,
};

pub mod create;
pub mod delete;
pub mod edit;

pub async fn run(
    options: crate::event_handler::command_handler::CommandOptions,
    _command: &serenity::model::application::interaction::application_command::ApplicationCommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let (cmd_name, cmd_options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;
    match cmd_name.as_str() {
        "create" => create::run(cmd_options, _command, &http).await?,
        "delete" => delete::run(cmd_options, _command, &http).await?,
        &_ => bail!("Unknown item config subcommand."),
    }
    Ok(())
}

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("item")
        .description("Configure items.")
        .kind(CommandOptionType::SubCommandGroup)
        .add_sub_option(create::option())
        .add_sub_option(delete::option());
    option
}
