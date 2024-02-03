use crate::{ db::{ models::Inventory, CLIENT }, event_handler::command_handler::CommandOptions };
use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::db::models::Item;

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?;
    let item_name = options
        .get_string_value(NAME_OPTION_NAME)
        .transpose()?
        .ok_or_else(|| anyhow!("No item name was found"))?;
    let item = Item::try_from_name(guild_id.into(), item_name.clone()).await?;

    let client = CLIENT.get().await;

    let mut session = client.start_session(None).await?;

    ({
        // The question marks within this scope will return the error only to the outer scope. Not
        // the entire function.
        Inventory::purge_item(guild_id.into(), &item_name, Some(&mut session)).await?;
        Item::delete_item(item, Some(&mut session)).await?;
        anyhow::Ok(()) // <-- this is needed because the scope cannot tell I meant an anyhow Error.
    })?;
    session.commit_transaction().await?; // finally, do the thing

    command.edit_response(http, EditInteractionResponse::new().content("Item deleted.")).await?;
    Ok(())
}

const NAME_OPTION_NAME: &str = "name";

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "delete",
        "Delete an item."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            NAME_OPTION_NAME,
            "The name of the item to delete."
        ).required(true)
    )
}
