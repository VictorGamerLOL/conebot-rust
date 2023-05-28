pub mod create;

use serenity::{
    builder::CreateApplicationCommand,
    http::Http,
    model::prelude::application_command::{ApplicationCommandInteraction, CommandDataOption},
};

pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync,
) {
    let cmd_name = options[0].name.as_str();
    match cmd_name {
        "create" => create::run(&options[0].options, command, http).await,
        _ => eprintln!("Unknown subcommand: {}", cmd_name),
    }
}

pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command
        .name("currency")
        .description("Commands related to managing currencies.")
        .dm_permission(false)
        .add_option(create::option());
    command
}
