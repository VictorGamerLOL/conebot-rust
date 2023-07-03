use anyhow::Result;
use serenity::{
    builder::CreateApplicationCommand,
    http::Http,
    model::prelude::interaction::application_command::{
        ApplicationCommandInteraction, CommandDataOption,
    },
};

/// # Errors
/// Serenity stuff.
pub async fn run(
    _options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync,
) -> Result<()> {
    let future = command
        .edit_original_interaction_response(&http, |msg| msg.content("177013"))
        .await?;
    Ok(())
}

#[must_use]
pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command.name("test").description("aaa");
    command
}
