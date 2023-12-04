use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::CreateCommandOption,
    http::{ CacheHttp, Http },
};

pub mod create;
pub mod delete;
pub mod edit;
pub mod list;

pub async fn run(
    options: crate::event_handler::command_handler::CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let (cmd_name, cmd_options) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;
    match cmd_name.as_str() {
        "create" => create::run(cmd_options, command, &http).await?,
        "delete" => delete::run(cmd_options, command, &http).await?,
        "edit" => edit::run(cmd_options, command, &http).await?,
        "list" => list::run(cmd_options, command, &http).await?,
        &_ => bail!("Unknown item config subcommand."),
    }
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommandGroup, "item", "Configure items.")
        .add_sub_option(create::option())
        .add_sub_option(delete::option())
        .add_sub_option(edit::option())
        .add_sub_option(list::option())
}
