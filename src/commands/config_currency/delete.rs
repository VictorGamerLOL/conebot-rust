use crate::db::models::Currency;
use crate::db::uniques::DbGuildId;
use crate::event_handler::command_handler::CommandOptions;
use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Send + Sync + CacheHttp
) -> Result<()> {
    let currency_name = options
        .get_string_value("name")
        .transpose()?
        .ok_or_else(|| anyhow!("No currency name was found"))?;
    let currency = Currency::try_from_name(
        DbGuildId::from(command.guild_id.unwrap()),
        currency_name
    ).await?.ok_or_else(|| anyhow!("Currency not found"))?;
    Currency::delete_currency(currency).await?;
    command.edit_response(http, EditInteractionResponse::new().content("Currency deleted.")).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "delete",
        "Delete a currency."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            "name",
            "The name of the currency to delete."
        ).required(true)
    )
    // option
    //     .name("delete")
    //     .description("Delete a currency.")
    //     .kind(CommandOptionType::SubCommand)
    //     .create_sub_option(|o| {
    //         o.name("name")
    //             .description("The name of the currency to delete.")
    //             .kind(CommandOptionType::String)
    //             .required(true)
    //     });
    // option
}
