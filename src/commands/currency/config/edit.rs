use chrono::Duration;
use serenity::{
    model::prelude::{
        application_command::ApplicationCommandInteraction,
        command::CommandOptionType,
    },
    http::CacheHttp,
    http::Http,
    builder::CreateApplicationCommandOption,
};
use anyhow::{ anyhow, Result };

use crate::{ event_handler::command_handler::CommandOptions, db::models::Currency };

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
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

    let field_name = field_name.to_lowercase().trim().replace(' ', "_");

    let currency = Currency::try_from_name(
        guild_id.into(),
        currency_name.clone()
    ).await?.ok_or_else(|| anyhow!("Currency {} does not exist.", currency_name))?;

    let mut currency_ = currency.lock().await;

    let mut currency__ = currency_
        .as_mut()
        .ok_or_else(|| anyhow!("Currency {} is being used in breaking operation", currency_name))?;

    // You'll see soon why we need this.
    let mut possible_fut = None;

    match field_name.as_str() {
        "name" => {
            // Rust can't tell that I am dropping currency_ before I start the future so I have to
            // clone the Arc. It's just a pointer it shouldn't be too expensive, right? I think it doesn't
            // implement Copy because you need to explicitly say that you need to increment the reference
            // count.
            possible_fut = Some(Currency::update_name(currency.clone(), value.clone()));
        }
        //TODO: Generate nicer error messages for users.
        "symbol" => currency__.update_symbol(value.clone()).await?,
        "visible" => currency__.update_visible(value.parse()?).await?,
        "base" => currency__.update_base(value.parse()?).await?,
        "base_value" => currency__.update_base_value(value.parse().ok()).await?,
        "pay" => currency__.update_pay(value.parse()?).await?,
        "earn_by_chat" => currency__.update_earn_by_chat(value.parse()?).await?,
        "channels_is_whitelist" => currency__.update_channels_is_whitelist(value.parse()?).await?,
        "roles_is_whitelist" => currency__.update_roles_is_whitelist(value.parse()?).await?,
        "earn_min" => currency__.update_earn_min(value.parse()?).await?,
        "earn_max" => currency__.update_earn_max(value.parse()?).await?,
        "earn_timeout" =>
            currency__.update_earn_timeout(Duration::seconds(value.parse::<i64>()?)).await?,
        "channels_whitelist" | "channels_blacklist" | "roles_blacklist" | "roles_whitelist" =>
            anyhow::bail!("List field is not editable with this command"),
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
    command.edit_original_interaction_response(http, |m| {
        m.content(format!("{}'s {} field has been updated to {}", currency_name, field_name, value))
    }).await?;

    Ok(())
}

const CURRENCY_OPTION_NAME: &str = "currency";
const FIELD_OPTION_NAME: &str = "field";
const VALUE_OPTION_NAME: &str = "value";

#[must_use]
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("edit")
        .description("Edit a currency's configuration given a field name.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|o| {
            o.name(CURRENCY_OPTION_NAME)
                .description("The currency to edit.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(FIELD_OPTION_NAME)
                .description("The field to edit.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(VALUE_OPTION_NAME)
                .description("The value to set the field to.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}