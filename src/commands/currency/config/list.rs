use serenity::{
    builder::{ CreateApplicationCommandOption, CreateEmbed },
    model::prelude::{
        command::CommandOptionType,
        application_command::ApplicationCommandInteraction,
    },
    http::{ Http, CacheHttp },
};
use anyhow::{ Result, anyhow };

use crate::{
    event_handler::command_handler::CommandOptions,
    db::models::{ Currency, ToKVs },
    commands::currency,
};

const COMMAND_OPTION_CURRENCY: &str = "currency";

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Clone + CacheHttp
) -> Result<()> {
    let currency = options
        .get_string_value(COMMAND_OPTION_CURRENCY)
        .ok_or_else(|| anyhow!("Could not find currency."))??;

    let currency = Currency::try_from_name(
        command.guild_id.ok_or_else(|| anyhow!("Command may not be performed in DMs"))?.into(),
        currency.clone()
    ).await?.ok_or_else(move || anyhow!("Currency {} does not exist.", currency))?;

    let mut embed = CreateEmbed::default();

    let currency = currency.lock().await;

    let mut currency_ = currency.try_to_kvs()?.into_iter();

    embed.title(
        format!(
            "Config for {}",
            currency_.next().ok_or_else(|| anyhow!("Invalid currency object"))?.1
        )
    );

    for (k, v) in currency_ {
        //TODO: Add prettifier for some of the embed fields.
        embed.field(k, v, true);
    }

    drop(currency);

    command.edit_original_interaction_response(http, |m| { m.add_embed(embed) }).await?;

    Ok(())
}

#[must_use]
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("list")
        .kind(CommandOptionType::SubCommand)
        .description("List out all of the config values for the specified currency.")
        .create_sub_option(|o| {
            o.name(COMMAND_OPTION_CURRENCY)
                .description("The currency to list the config values for.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}
