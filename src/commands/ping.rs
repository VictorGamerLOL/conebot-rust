use serenity::{
    builder::CreateApplicationCommand,
    http::Http,
    model::prelude::interaction::application_command::{
        ApplicationCommandInteraction, CommandDataOption,
    },
};

pub async fn run(
    _options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + std::marker::Send + std::marker::Sync,
) {
    let future = command.edit_original_interaction_response(&http, |msg| msg.content("Pong!"));
    match future.await {
        Ok(_) => (),
        Err(e) => eprintln!("Error: {}", e),
    };
}

pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command.name("ping").description("Pong!");
    command
}
