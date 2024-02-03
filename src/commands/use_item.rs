use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    client::Context,
    constants::MESSAGE_CODE_LIMIT,
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ Inventory, Item },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
    mechanics::item_action_handler::use_item,
};

use std::borrow::Cow;

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const AMOUNT_OPTION_NAME: &str = "amount";

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?;
    let item_name = options
        .get_string_value(ITEM_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No item name provided."))??;
    let amount = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .unwrap_or(IntOrNumber::Int(1))
        .cast_to_i64();

    let user_inventory = Inventory::try_from_user(guild_id.into(), command.user.id.into()).await?;
    let mut user_inventory = user_inventory.lock().await;
    let user_inventory_ = user_inventory
        .as_mut()
        .ok_or_else(|| anyhow!("User's inventory is being used in a breaking operation."))?;

    let item = Item::try_from_name(guild_id.into(), item_name.clone()).await?;

    let entry = user_inventory_
        .get_item(&item_name)
        .ok_or_else(|| anyhow!("You do not have that item."))?;
    if entry.amount() < amount {
        bail!("You do not have enough of that item.");
    }

    let mut response_content = String::new();

    //TODO: make the response not be just the response messages one after another.
    let use_result = use_item(command.user.id, user_inventory_, item, amount, 0, http).await?;

    if !use_result.success {
        response_content.push_str(
            use_result.message.unwrap_or(Cow::Borrowed("Failed to use item.")).as_ref()
        );
    } else {
        user_inventory_.take_item(&item_name, amount, None).await?;
        response_content.push_str(
            use_result.message.unwrap_or_else(|| Cow::Owned(format!("Used {}", item_name))).as_ref()
        );
    }

    // keep only the first 2000 chars
    response_content = response_content.chars().take(MESSAGE_CODE_LIMIT).collect::<String>();

    drop(user_inventory);
    command.edit_response(http, EditInteractionResponse::new().content(response_content)).await?;
    Ok(())
}

pub fn command() -> CreateCommand {
    CreateCommand::new("use-item")
        .description("Use an item.")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ITEM_NAME_OPTION_NAME,
                "The name of the item to use."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                AMOUNT_OPTION_NAME,
                "The amount of the item to use."
            ).required(false)
        )
}
