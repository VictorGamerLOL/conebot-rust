use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::CreateCommandOption,
    http::{ CacheHttp, Http },
};

use crate::event_handler::command_handler::CommandOptions;

pub async fn run(
    command: &CommandInteraction,
    options: CommandOptions,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let item_name = options
        .get_string_value("item_name")
        .ok_or_else(|| anyhow!("Could not find item name."))??;
    todo!()
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
