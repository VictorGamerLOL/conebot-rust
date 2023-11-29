use anyhow::{ anyhow, bail, Result };
use serenity::{
    builder::CreateCommandOption,
    http::{ CacheHttp, Http },
    all::{ CommandOptionType, CommandInteraction },
};

pub mod create;
pub mod delete;
pub mod edit;

pub async fn run(
    options: crate::event_handler::command_handler::CommandOptions,
    _command: &CommandInteraction,
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

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "item", "Configure items.")
        .add_sub_option(create::option())
        .add_sub_option(delete::option())
}
