use anyhow::{ anyhow, Result };
use serenity::{ all::CommandInteraction, builder::CreateCommand, client::Context };

use crate::event_handler::command_handler::CommandOptions;

pub mod add_entry;
pub mod create;
pub mod delete;
pub mod delete_entry;
pub mod view_table;

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let (cmd_name, cmd_options): (String, CommandOptions) = options
        .get_subcommand_args_and_name()
        .ok_or_else(|| anyhow!("Provided argument does not contain a subcommand."))?;
    match cmd_name.as_str() {
        "create" => create::run(cmd_options, command, http).await?,
        "add_entry" => add_entry::run(cmd_options, command, http).await?,
        "delete" => delete::run(cmd_options, command, http).await?,
        "delete_entry" => delete_entry::run(cmd_options, command, http).await?,
        "view_table" => view_table::run(cmd_options, command, http).await?,
        &_ => anyhow::bail!("Unknown config subcommand."),
    }
    Ok(())
}

pub fn command() -> CreateCommand {
    CreateCommand::new("config_drop_table")
        .description("Configure drop tables.")
        .dm_permission(false)
        .add_option(create::option())
        .add_option(add_entry::option())
        .add_option(delete::option())
        .add_option(delete_entry::option())
        .add_option(view_table::option())
}
