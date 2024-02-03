use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    client::Context,
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ Inventory, Item },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let member = options
        .get_user_value("member")
        .ok_or_else(|| anyhow!("No member was provided."))??;
    let item_name = options
        .get_string_value("item_name")
        .ok_or_else(|| anyhow!("No item name was provided."))??;
    let amount = options
        .get_int_or_number_value("amount")
        .transpose()?
        .unwrap_or(IntOrNumber::Int(1))
        .cast_to_i64();
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Cannot use commands in DMs."))?;

    // just checking if the item exists.
    let item = Item::try_from_name(guild_id.into(), item_name.clone()).await?;

    let member_inv = Inventory::try_from_user(guild_id.into(), member.into()).await?;
    let mut member_inv = member_inv.lock().await;
    let member_inv_ = member_inv
        .as_mut()
        .ok_or_else(|| anyhow!("Member's inventory is being used in a breaking operation."))?;

    member_inv_.give_item(item, amount, None, 0, http).await?;

    drop(member_inv);

    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!("Gave the user {} amount of {}", item_name, amount)
        )
    ).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "item", "Give an item to a user.")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "member",
                "The member to give the item to."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "item_name",
                "The name of the item to give."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "amount",
                "The amount of the item to give."
            ).required(false)
        )
}
