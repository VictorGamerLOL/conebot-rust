use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::store::Store,
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

/// Runs the command to edit an entry in the store.
///
/// # Errors
///
/// This function can return an `anyhow::Error` if there is an error editing the entry.
pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be performed in DMs."))?;
    let item_name: String = options
        .get_string_value(ITEM_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No item name provided."))??;
    let currency_name: String = options
        .get_string_value(CURRENCY_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No currency name provided."))??;
    let value: Option<f64> = options
        .get_int_or_number_value(VALUE_OPTION_NAME)
        .transpose()?
        .map(IntOrNumber::cast_to_f64);
    let amount: Option<i64> = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .map(IntOrNumber::cast_to_i64);

    if value.is_none() && amount.is_none() {
        anyhow::bail!("No value or amount provided.");
    }
    if value.is_some() && amount.is_some() {
        anyhow::bail!("Both value and amount provided.");
    }

    let store = Store::try_from_guild(guild_id.into()).await?;
    let mut store = store.write().await;
    let store_ = store
        .as_mut()
        .ok_or_else(|| anyhow!("Store is being used in a breaking operation."))?;

    if let Some(value) = value {
        if value < 0.0 {
            anyhow::bail!("Value cannot be negative.");
        }
        store_.edit_entry_value(&item_name, &currency_name, value, None).await?;
    } else if let Some(amount) = amount {
        if amount < 0 {
            anyhow::bail!("Amount cannot be negative.");
        }
        store_.edit_entry_amount(&item_name, &currency_name, amount, None).await?;
    }

    drop(store);

    command.edit_response(http, EditInteractionResponse::new().content("Entry edited.")).await?;

    Ok(())
}

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const CURRENCY_NAME_OPTION_NAME: &str = "currency_name";
const VALUE_OPTION_NAME: &str = "value";
const AMOUNT_OPTION_NAME: &str = "amount";

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "edit_entry",
        "Edit an entry in the store."
    )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ITEM_NAME_OPTION_NAME,
                "The name of the item to edit."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_NAME_OPTION_NAME,
                "The name of the currency to edit."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                VALUE_OPTION_NAME,
                "The value of the item in the currency."
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                AMOUNT_OPTION_NAME,
                "The amount of the item to give per sale."
            ).required(false)
        )
}
