use anyhow::{ anyhow, Result };
use chrono::Duration;
use serenity::all::{ CommandInteraction, CommandOptionType };
use serenity::builder::EditInteractionResponse;
use serenity::http::CacheHttp;
use serenity::{ builder::CreateCommandOption, http::Http };

use crate::db::{ models::currency::builder::Builder, uniques::DbGuildId };
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
    command: &CommandInteraction,
    http: impl AsRef<Http> + Send + Sync + CacheHttp
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
    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!("Made currency {symbol}{name}", symbol = symbol, name = name)
        )
    ).await?;
    Ok(())
}
// There might be a more efficient and compact way to do this but I cannot think of it right now.

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "create", "Create a new currency")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "name",
                "The name of the new currency"
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "symbol",
                "The symbol this currency will have"
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "visible",
                "If the currency is visible to non-staff"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "base",
                "If this will be the new base currency"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                "base_value",
                "Value of currency in terms of the base one"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "pay",
                "If members can pay each other this"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "earn_by_chat",
                "If members can earn this by chatting"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "channels_is_whitelist",
                "If channel restrictions are in whitelist mode (true) or blacklist mode (false)"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                "roles_is_whitelist",
                "If role restrictions are in whitelist mode (true) or blacklist mode (false)"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                "earn_min",
                "Minimum amount of currency earned per message"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                "earn_max",
                "Maximum amount of currency earned per message"
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "earn_timeout",
                "Cooldown in seconds between earning currency"
            ).required(false)
        )
}
