use anyhow::{ anyhow, bail, Result };
use core::str::FromStr;
use serenity::{
    builder::CreateApplicationCommandOption,
    http::{ CacheHttp, Http },
    model::prelude::{
        application_command::ApplicationCommandInteraction,
        command::CommandOptionType,
        ChannelId,
        GuildId,
        Mention,
        RoleId,
    },
};
use tokio::sync::MutexGuard;

use crate::{
    db::{ id::DbGuildId, models::Currency, ArcTokioRwLockOption },
    event_handler::command_handler::CommandOptions,
};

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let currency_name = options
        .get_string_value(CURRENCY_OPTION_NAME)
        .ok_or_else(|| anyhow!("Currency name not found."))??;
    let field_name = options
        .get_string_value(FIELD_OPTION_NAME)
        .ok_or_else(|| anyhow!("Field name not found."))??
        .trim()
        .to_lowercase()
        .replace(' ', "_");
    let operation = options
        .get_string_value(OPERATION_OPTION_NAME)
        .ok_or_else(|| anyhow!("Operation not found."))??
        .trim()
        .to_lowercase();
    let value = options
        .get_string_value(VALUE_OPTION_NAME)
        .ok_or_else(|| anyhow!("Value not found."))??;
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command can't be performed in DMs."))?;

    let mut currency = Currency::try_from_name(guild_id.into(), currency_name).await?.ok_or_else(||
        anyhow!("Currency not found.")
    )?;

    let mut currency = currency.write().await;

    let mut currency_ = currency
        .as_mut()
        .ok_or_else(|| anyhow!("Currency is being used in breaking operation."))?;
    if operation.as_str() == "clear" {
        clear(currency_, &field_name).await?;
        command.edit_original_interaction_response(&http, |m| {
            m.content(format!("Cleared {}", field_name))
        }).await?;
        return Ok(());
    }
    let mut value = Mention::from_str(&value)?;
    match operation.as_str() {
        "add" => add(currency_, &field_name, value).await?,
        "remove" => remove(currency_, &field_name, value).await?,
        _ => bail!("Invalid operation."),
    }
    drop(currency);
    command.edit_original_interaction_response(&http, |m| {
        m.content(format!("{}ed {} from/to {}", operation, value, field_name))
    }).await?;
    Ok(())
}

async fn add(currency: &mut Currency, field_name: &str, value: Mention) -> Result<()> {
    match field_name {
        "roles_whitelist" => {
            if let Mention::Role(r) = value {
                currency.add_whitelisted_role(r.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "roles_blacklist" => {
            if let Mention::Role(r) = value {
                currency.add_blacklisted_role(r.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "channels_whitelist" => {
            if let Mention::Channel(c) = value {
                currency.add_whitelisted_channel(c.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "channels_blacklist" => {
            if let Mention::Channel(c) = value {
                currency.add_blacklisted_channel(c.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        _ => bail!("Invalid field name."),
    }
    Ok(())
}

async fn remove(currency: &mut Currency, field_name: &str, value: Mention) -> Result<()> {
    match field_name {
        "roles_whitelist" => {
            if let Mention::Role(r) = value {
                currency.remove_whitelisted_role(&r.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "roles_blacklist" => {
            if let Mention::Role(r) = value {
                currency.remove_blacklisted_role(&r.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "channels_whitelist" => {
            if let Mention::Channel(c) = value {
                currency.remove_whitelisted_channel(&c.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        "channels_blacklist" => {
            if let Mention::Channel(c) = value {
                currency.remove_blacklisted_channel(&c.into(), None).await?;
            } else {
                bail!("Invalid value type for field {}", field_name);
            }
        }
        _ => bail!("Invalid field name."),
    }
    Ok(())
}

async fn clear(currency: &mut Currency, field_name: &str) -> Result<()> {
    match field_name {
        "roles_whitelist" => currency.overwrite_whitelisted_roles(vec![], None).await?,
        "roles_blacklist" => currency.overwrite_blacklisted_roles(vec![], None).await?,
        "channels_whitelist" => currency.overwrite_whitelisted_channels(vec![], None).await?,
        "channels_blacklist" => currency.overwrite_blacklisted_channels(vec![], None).await?,
        _ => bail!("Invalid field name."),
    }
    Ok(())
}

const CURRENCY_OPTION_NAME: &str = "currency";
const FIELD_OPTION_NAME: &str = "field";
const OPERATION_OPTION_NAME: &str = "operation";
const VALUE_OPTION_NAME: &str = "value";

#[must_use]
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("edit_list")
        .description("Edit a configuration field that is a field of a list.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|o| {
            o.name(CURRENCY_OPTION_NAME)
                .description("The currency to edit.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(FIELD_OPTION_NAME)
                .description("The list field to edit.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(OPERATION_OPTION_NAME)
                .description("The operation to perform on the list.")
                .kind(CommandOptionType::String)
                .required(true)
                .add_string_choice("add", "add")
                .add_string_choice("remove", "remove")
                .add_string_choice("clear", "clear")
        })
        .create_sub_option(|o| {
            o.name(VALUE_OPTION_NAME)
                .description("The value to add/remove to the list. Type whatever if clearing.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}
