use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    client::Context,
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ store::Store, Currency, Item },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const CURRENCY_NAME_OPTION_NAME: &str = "currency_name";
const VALUE_OPTION_NAME: &str = "value";
const AMOUNT_OPTION_NAME: &str = "amount";

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
    let value: f64 = options
        .get_int_or_number_value(VALUE_OPTION_NAME)
        .ok_or_else(|| anyhow!("No value provided."))?
        .map(IntOrNumber::cast_to_f64)?;
    let amount: i64 = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .map_or(1, IntOrNumber::cast_to_i64);

    Currency::try_from_name(guild_id.into(), currency_name.clone()).await?;
    Item::try_from_name(guild_id.into(), item_name.clone()).await?;

    let mut store = Store::try_from_guild(guild_id.into()).await?;
    let mut store = store.write().await;

    let mut store_ = store
        .as_mut()
        .ok_or_else(|| anyhow!("Store is being used in a breaking operation."))?;

    store_.add_entry(item_name, currency_name, value, amount, None).await?;

    drop(store);

    command.edit_response(
        http,
        EditInteractionResponse::new().content("Successfully added entry to store.")
    ).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "create_entry",
        "Create a new entry in the store."
    )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ITEM_NAME_OPTION_NAME,
                "The name of the item to add."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_NAME_OPTION_NAME,
                "The name of the currency to add."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                VALUE_OPTION_NAME,
                "The value of the item in the currency."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                AMOUNT_OPTION_NAME,
                "The amount of the item to give per sale."
            ).required(false)
        )
}
