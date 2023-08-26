pub mod create;
mod delete;
pub mod config;

use anyhow::{ anyhow, Result };
use serenity::{
    builder::CreateApplicationCommand,
    http::{ Http, CacheHttp },
    model::prelude::application_command::{ ApplicationCommandInteraction, CommandDataOption },
};

use crate::event_handler::command_handler::CommandOptions;

/// # Errors
/// Serenity stuff.
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let cmd_name = options[0].name.as_str();
    let cmd_options: CommandOptions = options[0].options.clone().into();
    match cmd_name {
        "create" => create::run(cmd_options, command, http.clone()).await?,
        "delete" => delete::run(&options[0].options, command, http.clone()).await?,
        "config" => config::run(cmd_options, command, &http).await?,
        _ => {
            return Err(anyhow!("Unknown subcommand: {}", cmd_name));
        }
    }
    Ok(())
}

#[must_use]
pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command
        .name("currency")
        .description("Commands related to managing currencies.")
        .dm_permission(false)
        .add_option(create::option())
        .add_option(delete::option())
        .add_option(config::option());
    command
}
