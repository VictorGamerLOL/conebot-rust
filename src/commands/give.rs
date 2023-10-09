use anyhow::{ anyhow, Result };
use serenity::{
    builder::CreateApplicationCommand,
    http::{ CacheHttp, Http },
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction,
                CommandDataOption,
                CommandDataOptionValue,
            },
            command::CommandOptionType,
        },
        user::User,
        Permissions,
    },
};

use crate::{
    db::models::{ Balances, Currency },
    event_handler::command_handler::CommandOptions,
    util::currency::truncate_2dp,
};

/// # Errors
/// TODO
///
/// # Panics
/// will not
#[allow(clippy::unused_async)]
#[allow(clippy::cast_precision_loss)]
pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let mut amount: f64 = options
        .get_int_or_number_value("amount")
        .ok_or_else(|| anyhow!("No amount was provided."))??
        .cast_to_f64();
    let mut currency = options
        .get_string_value("currency")
        .ok_or_else(|| anyhow!("No currency name was provided."))??;
    let (mut member, _) = options
        .get_user_value("member")
        .ok_or_else(|| anyhow!("No member was provided."))??;

    // I know the options are required, but you never know.
    if currency == String::new() {
        return Err(anyhow!("No currency name was provided."));
    } else if amount == 0.0 {
        return Err(anyhow!("No amount was provided."));
    } else if member == User::default() {
        return Err(anyhow!("No member was provided."));
    }

    amount = truncate_2dp(amount);

    if Currency::try_from_name(command.guild_id.unwrap().into(), currency.clone()).await?.is_none() {
        return Err(anyhow!("Currency {} does not exist.", currency));
    }

    if
        command.guild_id
            .ok_or_else(|| anyhow!("Cannot use commands in DMs."))?
            .member(http.clone(), member.id).await
            .is_err()
    {
        return Err(anyhow!("Member {} does not exist.", member.id));
    }

    let mut balances = Balances::try_from_user(
        command.guild_id.unwrap().into(),
        member.id.into()
    ).await?;
    let mut balances = balances.lock().await;

    let Some(mut balances_) = balances.as_mut() else {
        return Err(anyhow!("{}'s balances are being used in a breaking operation.", member.id));
    };

    let mut balance = balances_
        .balances_mut()
        .iter_mut()
        .find(|b| b.curr_name == currency);

    if balance.is_none() {
        balances_.create_balance(currency.clone()).await?;
        balance = balances_
            .balances_mut()
            .iter_mut()
            .find(|b| b.curr_name == currency);
    }

    let Some(mut balance) = balance else {
        return Err(
            anyhow!(
                "{}'s balance for {} has been created but for some reason we could not find it afterwards. Strange...",
                member.id,
                currency
            )
        );
    };

    balance.add_amount_unchecked(amount).await?;

    drop(balances);

    command.edit_original_interaction_response(http, |m| {
        m.content(format!("{} has been given {} of {}.", member.name, amount, currency))
    }).await?;
    Ok(())
}

#[must_use]
pub fn application_command() -> CreateApplicationCommand {
    let perms = Permissions::MANAGE_GUILD;
    let mut command = CreateApplicationCommand::default();
    command
        .name("give")
        .description("Give a user an amount of a currency.")
        .dm_permission(false)
        .default_member_permissions(perms)
        .create_option(|o| {
            o.name("currency")
                .description("The currency to give.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_option(|o| {
            o.name("amount")
                .description("The amount to give.")
                .kind(CommandOptionType::Number)
                .required(true)
        })
        .create_option(|o| {
            o.name("member")
                .description("The member to give the currency to.")
                .kind(CommandOptionType::User)
                .required(true)
        });
    command
}
