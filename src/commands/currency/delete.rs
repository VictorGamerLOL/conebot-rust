use crate::db::models::Currency;
use crate::db::id::DbGuildId;
use crate::event_handler::command_handler::CommandOptions;
use anyhow::{ anyhow, Result };
use serenity::builder::CreateApplicationCommandOption;
use serenity::http::Http;
use serenity::model::prelude::application_command::{
    ApplicationCommandInteraction,
    CommandDataOption,
};
use serenity::model::prelude::command::CommandOptionType;

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync
) -> Result<()> {
    let currency_name = options
        .get_string_value("name")
        .transpose()?
        .ok_or_else(|| anyhow!("No currency name was found"))?;
    let mut currency = Currency::try_from_name(
        DbGuildId::from(command.guild_id.unwrap()),
        currency_name
    ).await?.ok_or_else(|| anyhow!("Currency not found"))?;
    Currency::delete_currency(currency).await?;
    command.edit_original_interaction_response(http, |response|
        response.content("Currency deleted.")
    ).await?;
    Ok(())
}

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("delete")
        .description("Delete a currency.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|o| {
            o.name("name")
                .description("The name of the currency to delete.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}
