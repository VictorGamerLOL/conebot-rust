use std::sync::Arc;

use anyhow::{ anyhow, Result };
use chrono::Duration;
use serde_json::Value;
use serenity::model::prelude::application_command::CommandDataOptionValue;
use serenity::{
    builder::CreateApplicationCommandOption,
    http::Http,
    model::{
        prelude::interaction::application_command::ApplicationCommandInteraction,
        prelude::{ application_command::CommandDataOption, command::CommandOptionType },
    },
};
use tokio::sync::Mutex;

use crate::db::{ id::DbGuildId, models::currency::builder::Builder };

/// Runs the create currency subcommand.
///
/// # Errors
///
/// Returns an error if:
///
/// - Any of the options could not be resolved
/// - The currency name is empty
/// - The symbol is empty
/// - The currency already exists
///
/// # Panics
///
/// It shouldn't panic. This is done to please the linter.
#[allow(clippy::too_many_lines)] // Can't be asked.
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync
) -> Result<()> {
    let mut currency_builder: Builder = Builder::new(
        DbGuildId::from(command.guild_id.unwrap()), // This is safe because this command is guild only
        String::new(), // This will be set because it is a required option in the slash command
        String::new() // Same as above
    );
    let mut name = String::new();
    let mut symbol = String::new();
    for option in options.iter() {
        match option.name.as_str() {
            // The values of command options are serde_json Values which need to be converted to the correct Rust type.
            // A solid amount of this is just error handling.
            "name" => {
                name = match option.resolved.clone().ok_or(anyhow!("Failed to resolve name"))? {
                    CommandDataOptionValue::String(s) => s,
                    _ => {
                        return Err(anyhow!("Expected string but found something else"));
                    }
                };
                // remove trailing and leading whitespace from name
                name = name.trim().to_string();
                if name.is_empty() {
                    return Err(anyhow!("Currency name cannot be empty"));
                }
                currency_builder.curr_name(name.clone());
            }
            "symbol" => {
                symbol = option.value
                    .as_ref()
                    .ok_or(anyhow!("Symbol value not found"))?
                    .as_str()
                    .ok_or(anyhow!("Failed to convert symbol value to str"))?
                    .to_string();
                symbol = symbol.trim().to_string();
                if symbol.is_empty() {
                    return Err(anyhow!("Symbol cannot be empty"));
                }
                currency_builder.symbol(symbol.clone());
            }
            "visible" => {
                currency_builder.visible(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Visible value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse visible value to bool"))?
                );
            }
            "base" => {
                currency_builder.base(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Base value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse base value to bool"))?
                );
            }
            "base_value" => {
                currency_builder.base_value(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Base_value value provided but not found"))?
                        .as_f64()
                        .ok_or(anyhow!("Failed to parse base_value value to f64"))?
                );
            }
            "pay" => {
                currency_builder.pay(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Pay value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse pay value to bool"))?
                );
            }
            "earn_by_chat" => {
                currency_builder.earn_by_chat(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Each_by_chat value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse earn_by_chat value to bool"))?
                );
            }
            "channels_is_whitelist" => {
                currency_builder.channels_is_whitelist(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Channels_is_whitelist value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse channels_is_whitelist value to bool"))?
                );
            }
            "roles_is_whitelist" => {
                currency_builder.roles_is_whitelist(
                    option.value
                        .as_ref()
                        .ok_or(anyhow!("Roles_is_whitelist value provided but not found"))?
                        .as_bool()
                        .ok_or(anyhow!("Failed to parse roles_is_whitelist value to bool"))?
                );
            }
            "earn_min" => {
                currency_builder.earn_min(option.value.as_ref().unwrap().as_f64().unwrap());
            }
            "earn_max" => {
                currency_builder.earn_max(option.value.as_ref().unwrap().as_f64().unwrap());
            }
            "earn_timeout" => {
                currency_builder.earn_timeout(
                    Duration::seconds(
                        option.value
                            .as_ref()
                            .ok_or(anyhow!("earn_timeout value provided but not found"))?
                            .as_i64()
                            .ok_or(anyhow!("Failed to parse earn_timeout value to i64"))?
                    )
                );
            }
            &_ => {}
        };
    }
    currency_builder.build().await?;
    command.edit_original_interaction_response(http, |m| {
        m.content(format!("Made currency {symbol}{name}"))
    }).await?;
    Ok(())
}
// There might be a more efficient and compact way to do this but I cannot think of it right now.

#[must_use]
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
                .description(
                    "If channel restrictions are in whitelist mode (true) or blacklist mode (false)"
                )
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("roles_is_whitelist")
                .description(
                    "If role restrictions are in whitelist mode (true) or blacklist mode (false)"
                )
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
