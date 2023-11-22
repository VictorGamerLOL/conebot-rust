use anyhow::{ anyhow, Result };
use chrono::Duration;
use serenity::model::prelude::application_command::CommandDataOptionValue;
use serenity::{
    builder::CreateApplicationCommandOption,
    http::Http,
    model::{
        prelude::interaction::application_command::ApplicationCommandInteraction,
        prelude::{ application_command::CommandDataOption, command::CommandOptionType },
    },
};

use crate::db::{ id::DbGuildId, models::currency::builder::Builder };
use crate::event_handler::command_handler::CommandOptions;

/// Runs the create currency subcommand.
///
/// # Errors
///
/// Returns an error if:
///
/// - Any of the options could not be resolved
/// - The currency name is empty
/// - The symbol is empty
/// - The currency already exists
///
/// # Panics
///
/// It shouldn't panic. This is done to please the linter.
#[allow(clippy::too_many_lines)] // Can't be asked.
pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync
) -> Result<()> {
    let mut currency_builder: Builder = Builder::new(
        DbGuildId::from(command.guild_id.unwrap()), // This is safe because this command is guild only
        String::new(), // This will be set because it is a required option in the slash command
        String::new() // Same as above
    );
    let mut name = options
        .get_string_value("name")
        .ok_or_else(|| anyhow!("Name value not found"))??;
    let mut symbol = options
        .get_string_value("symbol")
        .ok_or_else(|| anyhow!("Symbol value not found"))??;
    currency_builder.curr_name(name.clone());
    currency_builder.symbol(symbol.clone());
    currency_builder.visible(options.get_bool_value("visible").transpose()?);
    currency_builder.base(options.get_bool_value("base").transpose()?);
    currency_builder.base_value(
        options
            .get_int_or_number_value("base_value")
            .transpose()?
            .map(|n| n.cast_to_f64())
    );
    currency_builder.pay(options.get_bool_value("pay").transpose()?);
    currency_builder.earn_by_chat(options.get_bool_value("earn_by_chat").transpose()?);
    currency_builder.channels_is_whitelist(
        options.get_bool_value("channels_is_whitelist").transpose()?
    );
    currency_builder.roles_is_whitelist(options.get_bool_value("roles_is_whitelist").transpose()?);
    currency_builder.earn_min(
        options
            .get_int_or_number_value("earn_min")
            .transpose()?
            .map(|n| n.cast_to_f64())
    );
    currency_builder.earn_max(
        options
            .get_int_or_number_value("earn_max")
            .transpose()?
            .map(|n| n.cast_to_f64())
    );
    currency_builder.earn_timeout(
        options
            .get_int_or_number_value("earn_timeout")
            .transpose()?
            .map(|n| Duration::seconds(n.cast_to_i64()))
    );
    currency_builder.build().await?;
    command.edit_original_interaction_response(http, |m| {
        m.content(format!("Made currency {symbol}{name}"))
    }).await?;
    Ok(())
}
// There might be a more efficient and compact way to do this but I cannot think of it right now.

#[must_use]
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("create")
        .kind(CommandOptionType::SubCommand)
        .description("Create a new currency.")
        .create_sub_option(|o| {
            o.kind(CommandOptionType::String)
                .name("name")
                .description("The name of the new currency.")
                .required(true)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::String)
                .name("symbol")
                .description("The symbol this currency will have")
                .required(true)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("visible")
                .description("If the currency is visible to non-staff")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("base")
                .description("If this will be the new base currency")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("base_value")
                .description("Value of currency in terms of the base one")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("pay")
                .description("If members can pay each other this")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("earn_by_chat")
                .description("If members can earn this by chatting")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("channels_is_whitelist")
                .description(
                    "If channel restrictions are in whitelist mode (true) or blacklist mode (false)"
                )
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Boolean)
                .name("roles_is_whitelist")
                .description(
                    "If role restrictions are in whitelist mode (true) or blacklist mode (false)"
                )
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("earn_min")
                .description("Minimum amount of currency earned per message")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Number)
                .name("earn_max")
                .description("Maximum amount of currency earned per message")
                .required(false)
        })
        .create_sub_option(|o| {
            o.kind(CommandOptionType::Integer)
                .name("earn_timeout")
                .description("Cooldown in seconds between earning currency")
                .required(false)
        });
    option
}
