use std::borrow::Cow;

use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    client::Context,
};

use crate::{
    db::{ models::{ store::Store, Balances, Inventory, Item }, CLIENT },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

#[allow(clippy::cast_precision_loss)]
pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be performed in DMs."))?;
    let user_id = command.user.id;
    let item_name: String = options
        .get_string_value(ITEM_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No item name provided."))??;
    let currency_name: String = options
        .get_string_value(CURRENCY_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No currency name provided."))??;
    let amount: i64 = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .map_or(1, IntOrNumber::cast_to_i64);

    let store = Store::try_from_guild(guild_id.into()).await?;
    let store = store.read().await;
    let store_ = store
        .as_ref()
        .ok_or_else(|| anyhow!("Store is being used in a breaking operation."))?;

    let entry = store_
        .get_entry(&item_name, &currency_name)
        .ok_or_else(|| anyhow!("No such entry in the store."))?;

    let item = Item::try_from_name(guild_id.into(), entry.item_name().to_owned()).await?;

    let to_take = entry.value() * (amount as f64);

    let to_give = entry.amount() * amount;

    let balances = Balances::try_from_user(guild_id.into(), user_id.into()).await?;
    let mut balances = balances.lock().await;
    let balances_ = balances
        .as_mut()
        .ok_or_else(|| anyhow!("Balances are being used in a breaking operation."))?;

    let inventory = Inventory::try_from_user(guild_id.into(), user_id.into()).await?;
    let mut inventory = inventory.lock().await;
    let inventory_ = inventory
        .as_mut()
        .ok_or_else(|| anyhow!("Inventory is being used in a breaking operation."))?;

    let balance = balances_.ensure_has_currency(Cow::Borrowed(&currency_name)).await?;

    let mut session = CLIENT.get().await.start_session(None).await?;

    session.start_transaction(None).await?;

    if balance.amount() < to_take {
        anyhow::bail!("Not enough currency to buy the item.");
    }

    let res = {
        balance.sub_amount(to_take, Some(&mut session)).await?;
        inventory_.give_item(item, to_give, Some(&mut session), 0, http).await?;
        anyhow::Ok(())
    };

    drop(balances);
    drop(inventory);

    if let Err(e) = res {
        session.abort_transaction().await?;
        bail!("Error buying item: {}", e);
    }
    session.commit_transaction().await?;

    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!(
                "Successfully bought {} {} for {} {}",
                to_give,
                entry.item_name(),
                to_take,
                entry.curr_name()
            )
        )
    ).await?;

    drop(store);

    Ok(())
}

const ITEM_NAME_OPTION_NAME: &str = "item";
const CURRENCY_NAME_OPTION_NAME: &str = "currency";
const AMOUNT_OPTION_NAME: &str = "amount";

pub fn command() -> CreateCommand {
    CreateCommand::new("buy")
        .description("Buy an item from the store.")
        .dm_permission(false)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ITEM_NAME_OPTION_NAME,
                "The item to buy."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_NAME_OPTION_NAME,
                "The currency to buy the item with, if possible."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                AMOUNT_OPTION_NAME,
                "The amount of the item to buy."
            ).required(false)
        )
}
