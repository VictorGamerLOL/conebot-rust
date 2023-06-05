use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Duration;
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
) -> Result<()> {
    let mut currency_builder: CurrencyBuilder = CurrencyBuilder::new(
        DbGuildId::from(command.guild_id.unwrap()), // This is safe because this command is guild only
        "".to_string(), // This will be set because it is a required option in the slash command
        "".to_string(), // Same as above
    );
    let mut name: String = String::new();

    for option in options.iter() {
        match option.name.as_str() {
            // The values of command options are serde_json Values which need to be converted to the correct Rust type.
            // A solid amount of this is just error handling.
            "name" => {
                name = option
                    .value
                    .as_ref()
                    .ok_or(anyhow!("Name value not found"))?
                    .as_str()
                    .ok_or(anyhow!("Failed to convert name value to str"))?
                    .to_string();
                currency_builder.curr_name(name.clone());
            }
            "symbol" => {
                currency_builder.symbol(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Symbol value not found"))?
                        .as_str()
                        .ok_or(anyhow!("Failed to convert symbol value to str"))?
                        .to_string(),
                );
            }
            "visible" => {
                currency_builder.visible(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Visible value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse visible value to bool"))?,
                );
            }
            "base" => {
                currency_builder.base(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Base value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse base value to bool"))?,
                );
            }
            "base_value" => {
                currency_builder.base_value(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Base_value value provided but not found"))?
                        .as_f64()
                        .ok_or(anyhow!("Failed to parse base_value value to f64"))?,
                );
            }
            "pay" => {
                currency_builder.pay(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Pay value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse pay value to bool"))?,
                );
            }
            "earn_by_chat" => {
                currency_builder.earn_by_chat(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Each_by_chat value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse earn_by_chat value to bool"))?,
                );
            }
            "channels_is_whitelist" => {
                currency_builder.channels_is_whitelist(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!(
                            "Channels_is_whitelist value provided but not found"
                        ))?
                        .as_bool()
                        .ok_or(anyhow!(
                            "Failed to parse channels_is_whitelist value to bool"
                        ))?,
                );
            }
            "roles_is_whitelist" => {
                currency_builder.roles_is_whitelist(
                    option
                        .value
                        .as_ref()
                        .ok_or(anyhow!("Roles_is_whitelist value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse roles_is_whitelist value to bool"))?,
                );
            }
            "earn_min" => {
                currency_builder.earn_min(option.value.as_ref().unwrap().as_f64().unwrap());
            }
            "earn_max" => {
                currency_builder.earn_max(option.value.as_ref().unwrap().as_f64().unwrap());
            }
            "earn_timeout" => {
                currency_builder.earn_timeout(Duration::seconds(
                    option.value.as_ref().unwrap().as_i64().unwrap(),
                ));
            }
            &_ => {}
        };
    }
    currency_builder.build().await?;
    command
        .edit_original_interaction_response(http, |m| m.content(format! {"Made currency {}", name}))
        .await?;
    Ok(())
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
                .description("If members can pay each other this")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("earn_by_chat")
                .description("If members can earn this by chatting")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("channels_is_whitelist")
                .description("If channel restrictions are in whitelist mode (true) or blacklist mode (false)")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("roles_is_whitelist")
                .description("If role restrictions are in whitelist mode (true) or blacklist mode (false)")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("earn_min")
                .description("Minimum amount of currency earned per message")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("earn_max")
                .description("Maximum amount of currency earned per message")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Integer)
                .name("earn_timeout")
                .description("Cooldown in seconds between earning currency")
                .required(false)
        });
    option
}
