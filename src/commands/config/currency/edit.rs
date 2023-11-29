use anyhow::{ anyhow, Result };
use chrono::Duration;
use serenity::{
    http::CacheHttp,
    http::Http,
    model::prelude::{},
    all::{ CommandInteraction, CommandOptionType },
    builder::{ EditInteractionResponse, CreateCommandOption },
};

use crate::{ db::models::Currency, event_handler::command_handler::CommandOptions };

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Sync + Send + Clone + CacheHttp
) -> Result<()> {
    let currency_name = options
        .get_string_value(CURRENCY_OPTION_NAME)
        .ok_or_else(|| anyhow!("Currency name not found."))??;
    let field_name = options
        .get_string_value(FIELD_OPTION_NAME)
        .ok_or_else(|| anyhow!("Field name not found."))??;
    let value = options
        .get_string_value(VALUE_OPTION_NAME)
        .ok_or_else(|| anyhow!("Value not found."))??;
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command may not be performed in DMs"))?;

    let field_name = field_name.to_lowercase().trim().replace([' ', '-'], "_");

    let currency = Currency::try_from_name(
        guild_id.into(),
        currency_name.clone()
    ).await?.ok_or_else(|| anyhow!("Currency {} does not exist.", currency_name))?;

    let mut currency_ = currency.write().await;

    let mut currency__ = currency_
        .as_mut()
        .ok_or_else(|| {
            anyhow!("Currency {} is being used in breaking operation", currency_name)
        })?;

    // You'll see soon why we need this.
    let mut possible_fut = None;

    match field_name.as_str() {
        "name" => {
            // Rust can't tell that I am dropping currency_ before I start the future so I have to
            // clone the Arc. It's just a pointer it shouldn't be too expensive, right? I think it doesn't
            // implement Copy because you need to explicitly say that you need to increment the reference
            // count.
            possible_fut = Some(Currency::update_name(currency.clone(), value.clone(), None));
        }
        //TODO: Generate nicer error messages for users.
        "symbol" => currency__.update_symbol(&value, None).await?,
        "visible" => currency__.update_visible(value.parse()?, None).await?,
        "base" => currency__.update_base(value.parse()?, None).await?,
        "base_value" => {
            currency__.update_base_value(value.parse().ok(), None).await?;
        }
        "pay" => currency__.update_pay(value.parse()?, None).await?,
        "earn_by_chat" => currency__.update_earn_by_chat(value.parse()?, None).await?,
        "channels_is_whitelist" => {
            currency__.update_channels_is_whitelist(value.parse()?, None).await?;
        }
        "roles_is_whitelist" => {
            currency__.update_roles_is_whitelist(value.parse()?, None).await?;
        }
        "earn_min" => currency__.update_earn_min(value.parse()?, None).await?,
        "earn_max" => currency__.update_earn_max(value.parse()?, None).await?,
        "earn_timeout" => {
            currency__.update_earn_timeout(Duration::seconds(value.parse::<i64>()?), None).await?;
        }
        "channels_whitelist" | "channels_blacklist" | "roles_blacklist" | "roles_whitelist" => {
            anyhow::bail!("List field is not editable with this command");
        }
        _ => anyhow::bail!("Unknown field: {}", field_name),
    }

    drop(currency_);
    // this needs to be done because if the value is not dropped the future will never complete as it awaits
    // indefinitely to acquire the lock.

    // We also don't just let the end of scope drop it because it would have to be held across the response
    // edit which would add further delay.
    if let Some(fut) = possible_fut {
        fut.await?;
    }
    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!("{}'s {} field has been updated to {}", currency_name, field_name, value)
        )
    ).await?;

    Ok(())
}

const CURRENCY_OPTION_NAME: &str = "currency";
const FIELD_OPTION_NAME: &str = "field";
const VALUE_OPTION_NAME: &str = "value";

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "edit",
        "Edit a currency's configuration given a field name."
    )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_OPTION_NAME,
                "The currency to edit."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                FIELD_OPTION_NAME,
                "The field to edit."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                VALUE_OPTION_NAME,
                "The value to set the field to."
            ).required(true)
        )
}
