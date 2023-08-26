mod message;
pub mod command_handler;

use crate::commands;
use anyhow::anyhow;
use anyhow::Result;
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
// What the FUCK Rust?
use crate::event_handler::message::message;
use serenity::async_trait;
use serenity::client::EventHandler;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::Interaction;
use serenity::model::prelude::interaction::InteractionResponseType;
use serenity::model::prelude::{ Message, Ready };
use serenity::prelude::Context;
use tracing::{ error, info, instrument };

use self::command_handler::CommandOptions;
#[derive(Debug)]
pub struct Handler;

impl Handler {
    async fn handle_command<'a>(
        &self,
        command: &ApplicationCommandInteraction,
        ctx: &Context
    ) -> Result<()> {
        let mut options: CommandOptions = command.data.options.clone().into();
        match command.data.name.as_str() {
            "ping" => commands::ping::run(&command.data.options, command, ctx).await?,
            "test" => commands::test1::run(&command.data.options, command, ctx).await?,
            "currency" => commands::currency::run(&command.data.options, command, ctx).await?,
            "balance" => commands::balance::run(options, command, ctx).await?,
            "give" => commands::give::run(options, command, ctx).await?,
            "take" => commands::take::run(options, command, ctx).await?,
            _ => {
                return Err(anyhow!("Unknown command: {}", command.data.name));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip(self, _ctx), level = "debug")]
    async fn message(&self, _ctx: Context, _new_message: Message) {
        let _ = message(_ctx, _new_message).await;
    }

    /// This function is responsible for initializing the global application commands.
    ///
    /// # Panics
    ///
    /// If it fails to register the commands. If the commands are not registered bot as well might not work.
    #[instrument(skip_all)] // Required for tracing, since this would fill the output with a lot of junk if arguments were to be logged.
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is running!", ready.user.name);
        if
            let Err(e) = Command::set_global_application_commands(&ctx.http, |commands| {
                commands.set_application_commands(
                    vec![
                        commands::ping::application_command(),
                        commands::test1::application_command(),
                        commands::currency::application_command(),
                        commands::balance::application_command(),
                        commands::give::application_command()
                    ]
                )
            }).await
        {
            error!("Error registering commands: {}", e);
        };
    }

    /// This function is responsible for handling all incoming interactions.
    ///
    /// New commands must be entered here when added due to the nature of Rust.
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) -> () {
        if let Interaction::ApplicationCommand(command) = interaction {
            info!("Received command interaction: {:#?}", command.data.name);
            command
                .create_interaction_response(&ctx.http, |a| {
                    a.kind(
                        InteractionResponseType::DeferredChannelMessageWithSource
                    ).interaction_response_data(|msg| msg.ephemeral(true))
                }).await
                .unwrap_or_else(|e| error!("Error creating response: {}", e)); // This returns
            let res = self.handle_command(&command, &ctx).await;
            if let Err(e) = res {
                if let Some(e) = e.downcast_ref::<serenity::Error>() {
                    // If it's serenity's fault it is futile to try to respond to the user
                    error!("Serenity error: {}", e);
                } else if
                    let Err(e) = command // If it is not serenity's fault we can respond to the user
                        .edit_original_interaction_response(&ctx.http, |m| {
                            m.content(format!("An error occurred: {e}"))
                        }).await
                {
                    error!("Error editing response: {}", e); // Assuming serenity does not decide to error out now
                }
            }
        }
    }
}
