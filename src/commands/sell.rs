use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};
use tokio::join;

use crate::{
    db::{ models::{ Balances, Currency, Inventory, Item }, CLIENT },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be performed in DMs."))?;
    let user_id = command.user.id;
    let item = options
        .get_string_value("item")
        .transpose()?
        .ok_or_else(|| anyhow!("No item specified."))?;
    let amount = options
        .get_int_or_number_value("amount")
        .transpose()?
        .map_or(1, IntOrNumber::cast_to_i64);

    if amount < 1 {
        bail!("Amount must be greater than 0.");
    }

    let balance = Balances::try_from_user(guild_id.into(), user_id.into()).await?;
    let inventory = Inventory::try_from_user(guild_id.into(), user_id.into()).await?;
    let item = Item::try_from_name(guild_id.into(), item).await?;

    let (mut balance, mut inventory, item) = join!(balance.lock(), inventory.lock(), item.read());

    let balance_ = balance
        .as_mut()
        .ok_or_else(|| anyhow!("Balance is being used in a breaking operation."))?;
    let inventory_ = inventory
        .as_mut()
        .ok_or_else(|| anyhow!("Inventory is being used in a breaking operation."))?;
    let item_ = item
        .as_ref()
        .ok_or_else(|| anyhow!("Item is being used in a breaking operation."))?;

    if !item_.sellable() {
        bail!("Item cannot be sold.");
    }

    let entry = inventory_
        .get_item(item_.name())
        .ok_or_else(|| anyhow!("Item not found in inventory."))?;

    if entry.amount() < amount {
        bail!("Not enough items to sell.");
    }

    Currency::try_from_name(guild_id.into(), item_.currency_value().to_owned()).await?;

    let mut session = CLIENT.get().await.start_session(None).await?;

    session.start_transaction(None).await?;

    let income = item_.value() * (amount as f64);
    balance_
        .ensure_has_currency(std::borrow::Cow::Borrowed(item_.currency_value())).await?
        .add_amount(income, Some(&mut session)).await?;
    entry.sub_amount(amount, Some(&mut session)).await?;

    session.commit_transaction().await?;

    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!("Sold {}x {} for {} {}.", amount, item_.name(), income, item_.currency_value())
        )
    ).await?;
    Ok(())
}

pub fn command() -> CreateCommand {
    CreateCommand::new("sell")
        .description("Sell an item for currency.")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "item",
                "The item to sell."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "amount",
                "The amount of the item to sell."
            ).required(true)
        )
}
