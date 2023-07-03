pub mod create;
mod delete;
mod give;

use anyhow::{anyhow, Result};
use serenity::{
    builder::CreateApplicationCommand,
    http::Http,
    model::prelude::application_command::{ApplicationCommandInteraction, CommandDataOption},
};

/// # Errors
/// Serenity stuff.
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone,
) -> Result<()> {
    let cmd_name = options[0].name.as_str();
    match cmd_name {
        "create" => create::run(&options[0].options, command, http.clone()).await?,
        "delete" => delete::run(&options[0].options, command, http.clone()).await?,
        _ => return Err(anyhow!("Unknown subcommand: {}", cmd_name)),
    };
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
        .add_option(delete::option());
    command
}
