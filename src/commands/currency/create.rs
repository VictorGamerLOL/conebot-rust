use std::sync::Arc;

use serde_json::Value;
use serenity::{
    builder::CreateApplicationCommandOption,
    http::Http,
    model::{
        prelude::interaction::application_command::ApplicationCommandInteraction,
        prelude::{application_command::CommandDataOption, command::CommandOptionType},
    },
};
use tokio::sync::Mutex;

use crate::db::{id::DbGuildId, models::currency::currency_builder::CurrencyBuilder};

pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync,
) {
    let mut curr_builder: CurrencyBuilder = CurrencyBuilder::new(
        DbGuildId::from(command.guild_id.unwrap()), // This is safe because this command is guild only
        "".to_string(), // This will be set because it is a required option in the slash command
        "".to_string(), // Same as above
    );

    options.iter().for_each(|option| {
        match option.name.as_str() {
            // The values of command options are serde_json Values which need to be converted to the correct Rust type
            "name" => {
                curr_builder
                    .curr_name(option.value.as_ref().unwrap().as_str().unwrap().to_string());
            }
            "symbol" => {
                curr_builder.symbol(option.value.as_ref().unwrap().as_str().unwrap().to_string());
            }
            "visible" => {
                curr_builder.visible(option.value.as_ref().unwrap().as_bool().unwrap());
            }
            "base" => {
                curr_builder.base(option.value.as_ref().unwrap().as_bool().unwrap());
            }
            "base_value" => {
                curr_builder.base_value(option.value.as_ref().unwrap().as_f64().unwrap());
            }
            "pay" => {
                curr_builder.pay(option.value.as_ref().unwrap().as_bool().unwrap());
            }
            _ => (),
        };
    });
    todo!()
}

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("create")
        .kind(CommandOptionType::SubCommand)
        .description("Create a new currency.")
        .create_sub_option(|o| {
            o.kind(CommandOptionType::String)
                .name("name")
                .description("The name of the new currency.")
                .required(true)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::String)
                .name("symbol")
                .description("The symbol this currency will have")
                .required(true)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("visible")
                .description("If the currency is visible to non-staff")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("base")
                .description("If this will be the new base currency")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("base_value")
                .description("Value of currency in terms of the base one")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("pay")
                .description("If members can pay eachother this.")
                .required(false)
        });
    todo!();
}
