use std::sync::{Arc, Mutex};

use crate::commands; // What the FUCK Rust?
use lazy_static::lazy_static;
use serenity::async_trait;
use serenity::client::EventHandler;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::Interaction;
use serenity::model::prelude::interaction::InteractionResponseType;
use serenity::model::prelude::{Message, Ready};
use serenity::prelude::Context;

pub struct Handler;

struct UserTimeout {
    user_id: u64,
    guild: u64,
    currency: String,
}

lazy_static! {
    static ref USER_TIMEOUTS: Arc<Mutex<Vec<UserTimeout>>> /*dafuq?*/ = Arc::new(Mutex::new(Vec::new()));
} // Walmart cache

#[async_trait]
impl EventHandler for Handler {
    /// This function is responsible for initializing the global application commands.
    ///
    /// # Panics
    ///
    /// If it fails to register the commands. If the commands are not registered bot as well might not work.
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        Command::set_global_application_commands(&ctx.http, |commands| {
            commands.set_application_commands(vec![
                commands::ping::application_command(),
                commands::test1::application_command(),
            ])
        })
        .await
        .expect("Failed to register commands.");
    }

    /// This function is responsible for handling all incoming interactions.
    ///
    /// New commands must be entered here when added due to the nature of Rust.
    async fn interaction_create(&self, _ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            println!("Received command interaction: {:#?}", command.data.name);
            command
                .create_interaction_response(&_ctx.http, |a| {
                    a.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                        .interaction_response_data(|msg| msg.ephemeral(true))
                })
                .await
                .unwrap_or_else(|e| eprintln!("Error creating response: {}", e)); // This returns
            match command.data.name.as_str() {
                "ping" => commands::ping::run(&command.data.options, &command, &_ctx.http).await,
                "test" => commands::test1::run(&command.data.options, &command, &_ctx.http).await,
                _ => eprintln!("Unknown command: {}", command.data.name),
            }
        }
    }

    ///TODO: Idk yet I just have this.
    async fn message(&self, ctx: Context, new_message: Message) {
        let mut found: bool;
        todo!()
    }
}
