use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, CreateEmbed, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{ db::models::{ Item, ToKVs }, event_handler::command_handler::CommandOptions };

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    //TODO: Pretty print the output to the user.
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be performed in DMs."))?;

    let item_name = options
        .get_string_value("item_name")
        .ok_or_else(|| anyhow!("Could not find item name."))??;
    let mut embed = CreateEmbed::new();

    let mut item = Item::try_from_name(guild_id.into(), item_name).await?;
    let mut item = item.read().await;

    let mut item_ = item.as_ref().ok_or_else(|| anyhow!("Item not found."))?;

    for (key, value) in item_.try_to_kvs()? {
        embed = embed.field(key, value, true);
    }

    drop(item);

    command.edit_response(http, EditInteractionResponse::new().add_embed(embed)).await?;

    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "list",
        "List the configuration of an item."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            "item_name",
            "The name of the item to list the configuration of."
        ).required(true)
    )
}
