pub mod command_handler;
mod message;

use crate::commands;
use anyhow::anyhow;
use anyhow::Result;
use serenity::all::Command;
use serenity::all::CommandInteraction;
use serenity::all::Interaction;
use serenity::builder::CreateInteractionResponseMessage;
use serenity::builder::EditInteractionResponse;
// What the heck Rust?
use crate::event_handler::message::message;
use serenity::async_trait;
use serenity::client::EventHandler;
use serenity::model::prelude::{ Message, Ready };
use serenity::prelude::Context;
use tracing::{ error, info, instrument };

use self::command_handler::CommandOptions;
#[derive(Debug)]
pub struct Handler;

impl Handler {
    async fn handle_command<'a>(&self, command: &CommandInteraction, ctx: &Context) -> Result<()> {
        let options: CommandOptions = command.data.options.clone().into();
        match command.data.name.as_str() {
            "ping" => commands::ping::run(options, command, ctx).await?,
            "currency" => commands::currency::run(options, command, ctx).await?,
            "balance" => commands::balance::run(options, command, ctx).await?,
            "give" => commands::give::run(options, command, ctx).await?,
            "take" => commands::take::run(options, command, ctx).await?,
            "use-item" => commands::use_item::run(options, command, ctx).await?,
            "inv" => commands::inv::run(options, command, ctx).await?,
            "buy" => commands::buy::run(options, command, ctx).await?,
            "sell" => commands::sell::run(options, command, ctx).await?,
            "config_currency" => commands::config_currency::run(options, command, ctx).await?,
            "config_drop_table" => commands::config_drop_table::run(options, command, ctx).await?,
            "config_item" => commands::config_item::run(options, command, ctx).await?,
            "config_store" => commands::config_store::run(options, command, ctx).await?,
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
        if let Err(e) = message(_ctx, _new_message).await {
            error!("Error handling message: {}", e);
        }
        // e
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
            let Err(e) = Command::set_global_commands(
                &ctx.http,
                vec![
                    commands::ping::command(),
                    commands::currency::command(),
                    commands::balance::command(),
                    commands::give::command(),
                    commands::take::command(),
                    commands::config_currency::command(),
                    commands::config_drop_table::command(),
                    commands::config_item::command(),
                    commands::config_store::command(),
                    commands::use_item::command(),
                    commands::inv::command(),
                    commands::buy::command(),
                    commands::sell::command()
                ]
            ).await
        {
            error!("Error registering commands: {}", e);
        };
    }

    /// This function is responsible for handling all incoming interactions.
    ///
    /// New commands must be entered here when added due to the nature of Rust.
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) -> () {
        if let Interaction::Command(command) = interaction {
            info!("Received command interaction: {:#?}", command.data.name);
            command
                .create_response(
                    &ctx.http,
                    serenity::builder::CreateInteractionResponse::Defer(
                        CreateInteractionResponseMessage::new().ephemeral(true)
                    )
                ).await
                .unwrap_or_else(|e| error!("Error creating response: {}", e)); // This returns
            let res = self.handle_command(&command, &ctx).await;
            if let Err(e) = res {
                if let Some(e) = e.downcast_ref::<serenity::Error>() {
                    // If it's serenity's fault it is futile to try to respond to the user
                    error!("Serenity error: {}", e);
                } else if
                    let Err(e) = command.edit_response(
                        &ctx.http,
                        EditInteractionResponse::new().content(format!("Error: {e}"))
                    ).await
                {
                    error!("Error editing response: {}", e); // Assuming serenity does not decide to error out now
                }
            }
        }
    }
}
