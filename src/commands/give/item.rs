use anyhow::Result;
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::event_handler::command_handler::CommandOptions;

pub async fn run(
    _options: CommandOptions,
    _command: &CommandInteraction,
    _http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    _command.edit_response(_http, EditInteractionResponse::new().content("Unimplemented.")).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "item", "Give an item to a user.")
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
