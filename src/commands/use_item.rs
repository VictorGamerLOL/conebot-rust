use anyhow::Result;
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::event_handler::command_handler::CommandOptions;

const ITEM_NAME_OPTION_NAME: &str = "item_name";
const AMOUNT_OPTION_NAME: &str = "amount";

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    command.edit_response(http, EditInteractionResponse::new().content("Unimplemented.")).await?;
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
