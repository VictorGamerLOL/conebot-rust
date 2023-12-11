use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ Inventory, Item },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
    mechanics::item_action_handler::use_item,
};

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const AMOUNT_OPTION_NAME: &str = "amount";

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?;
    let item_name = options
        .get_string_value(ITEM_NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("No item name provided."))??;
    let mut amount = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .unwrap_or(IntOrNumber::Int(1))
        .cast_to_i64();

    let user_inventory = Inventory::from_user(guild_id.into(), command.user.id.into()).await?;
    let mut user_inventory = user_inventory.lock().await;
    let mut user_inventory_ = user_inventory
        .as_mut()
        .ok_or_else(|| anyhow!("User's inventory is being used in a breaking operation."))?;

    let item = Item::try_from_name(guild_id.into(), item_name.clone()).await?;
    let mut item = item.read().await;
    let item_ = item
        .as_ref()
        .ok_or_else(|| anyhow!("Item is being used in a breaking operation."))?;

    let mut response_content = String::new();

    //TODO: make the response not be just the response messages one after another.
    while amount > 0 {
        let use_result = use_item(command.user.id, item_, &http).await?;

        if !use_result.success {
            command.edit_response(
                http,
                EditInteractionResponse::new().content(
                    use_result.message.unwrap_or("Something went wrong.")
                )
            ).await?;
            return Ok(());
        }
        user_inventory_.take_item(&item_name, None).await?;
        response_content.push_str(use_result.message.unwrap_or(&format!("Used {}", item_name)));
        response_content.push('\n');
        amount -= 1;
    }

    drop(user_inventory);
    drop(item);
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
