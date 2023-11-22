use crate::event_handler::command_handler::CommandOptions;
use anyhow::{ anyhow, bail, Result };
use serenity::{
    builder::CreateApplicationCommandOption,
    http::{ CacheHttp, Http },
    model::application::{
        command::CommandOptionType,
        interaction::application_command::ApplicationCommandInteraction,
    },
};

use crate::db::models::Item;

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    _http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
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
    command.edit_original_interaction_response(_http, |response|
        response.content("Item deleted.")
    ).await?;
    Ok(())
}

const NAME_OPTION_NAME: &str = "name";

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("delete")
        .kind(CommandOptionType::SubCommand)
        .description("Delete an item.")
        .create_sub_option(|option| {
            option
                .name(NAME_OPTION_NAME)
                .description("The name of the item to delete.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}
