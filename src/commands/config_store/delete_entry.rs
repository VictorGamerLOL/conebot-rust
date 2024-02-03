use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{ db::models::store::Store, event_handler::command_handler::CommandOptions };

/// Runs the `delete_entry` command.
///
/// # Errors
///
/// This function can return an `anyhow::Error` if there is an error deleting the entry from the store.
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

    let store = Store::try_from_guild(guild_id.into()).await?;
    let mut store = store.write().await;

    let store_ = store
        .as_mut()
        .ok_or_else(|| anyhow!("Store is being used in a breaking operation."))?;

    store_.delete_entry(&item_name, &currency_name, None).await?;

    drop(store);

    command.edit_response(
        http,
        EditInteractionResponse::new().content("Successfully deleted entry from store.")
    ).await?;
    Ok(())
}

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const CURRENCY_NAME_OPTION_NAME: &str = "currency_name";

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "delete_entry",
        "Delete an entry from the store."
    )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ITEM_NAME_OPTION_NAME,
                "The name of the item from the entry to delete."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_NAME_OPTION_NAME,
                "The name of the currency from the entry to delete."
            ).required(true)
        )
}
