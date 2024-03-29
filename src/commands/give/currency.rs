use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType, UserId },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ Balances, Currency },
    event_handler::command_handler::CommandOptions,
    util::currency::truncate_2dp,
};

#[allow(clippy::unused_async)]
#[allow(clippy::cast_precision_loss)]
pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let mut amount: f64 = options
        .get_int_or_number_value("amount")
        .ok_or_else(|| anyhow!("No amount was provided."))??
        .cast_to_f64();
    let currency = options
        .get_string_value("currency-name")
        .ok_or_else(|| anyhow!("No currency name was provided."))??;
    let member = options
        .get_user_value("member")
        .ok_or_else(|| anyhow!("No member was provided."))??
        .to_user(&http).await?;

    // I know the options are required, but you never know.
    if currency == String::new() {
        return Err(anyhow!("No currency name was provided."));
    } else if amount == 0.0 {
        return Err(anyhow!("No amount was provided or provided amount was 0."));
    } else if member.id == UserId::default() {
        return Err(anyhow!("No member was provided."));
    }

    amount = truncate_2dp(amount);

    if
        Currency::try_from_name(command.guild_id.unwrap().into(), currency.clone()).await?.is_none()
    {
        return Err(anyhow!("Currency {} does not exist.", currency));
    }

    if
        command.guild_id
            .ok_or_else(|| anyhow!("Cannot use commands in DMs."))?
            .member(&http, member.id).await
            .is_err()
    {
        return Err(anyhow!("Member {} does not exist.", member.id));
    }

    let balances = Balances::try_from_user(
        command.guild_id.unwrap().into(),
        member.id.into()
    ).await?;
    let mut balances = balances.lock().await;

    let Some(balances_) = balances.as_mut() else {
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

    let Some(balance) = balance else {
        return Err(
            anyhow!(
                "{}'s balance for {} has been created but for some reason we could not find it afterwards. Strange...",
                member.id,
                currency
            )
        );
    };

    balance.add_amount_unchecked(amount, None).await?;

    drop(balances);

    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!("{} has been given {} of {}.", member.name, amount, currency)
        )
    ).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "currency", "Give the user currency.")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "currency-name",
                "The currency to give."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                "amount",
                "The amount to give."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "member",
                "The member to give the currency to."
            ).required(true)
        )
}
