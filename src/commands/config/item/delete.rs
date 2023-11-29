use crate::event_handler::command_handler::CommandOptions;
use anyhow::{ anyhow, bail, Result };
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
    let item_name = options
        .get_string_value(NAME_OPTION_NAME)
        .transpose()?
        .ok_or_else(|| anyhow!("No item name was found"))?;
    let mut item = Item::try_from_name(
        command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?.into(),
        item_name
    ).await?;

    Item::delete_item(item).await?;
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
