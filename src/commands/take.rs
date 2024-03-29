use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommand, CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
    model::{ user::User, Permissions },
};

use crate::{
    db::models::{ Balances, Currency },
    event_handler::command_handler::CommandOptions,
    util::currency::truncate_2dp,
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Clone + CacheHttp
) -> Result<()> {
    let amount = truncate_2dp(
        options
            .get_int_or_number_value("amount")
            .ok_or_else(|| anyhow!("Failed to find amount"))??
            .cast_to_f64()
    );
    let currency = options
        .get_string_value("currency")
        .ok_or_else(|| anyhow!("Failed to find currency"))??;
    let member: User = options
        .get_user_value("member")
        .ok_or_else(|| anyhow!("Failed to find member option."))??
        .to_user(&http).await?;
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command may not be performed in DMs"))?;

    if Currency::try_from_name(guild_id.into(), currency.clone()).await?.is_none() {
        return Err(anyhow!("Currency {} does not exist.", currency));
    }

    if guild_id.member(&http, member.id).await.is_err() {
        return Err(anyhow!("Member {} is not in guild {}", member, guild_id));
    }

    let balances = Balances::try_from_user(guild_id.into(), member.id.into()).await?;
    let mut balances = balances.lock().await;

    let Some(balances_) = balances.as_mut() else {
        return Err(anyhow!("{}'s balances are being used in a breaking operation.", member.name));
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
                member.name,
                currency
            )
        );
    };

    balance.sub_amount_unchecked(amount, None).await?;

    drop(balances);

    command.edit_response(
        &http,
        EditInteractionResponse::new().content(
            format!("{} has been taken from {} of {}", member.name, amount, currency)
        )
    ).await?;
    Ok(())
}

pub fn command() -> CreateCommand {
    let perms = Permissions::MANAGE_GUILD;
    CreateCommand::new("take")
        .description("Take away from a user a specified amount of a currency.")
        .dm_permission(false)
        .default_member_permissions(perms)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "currency",
                "The currency to take."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                "amount",
                "The amount to take."
            ).required(true)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "member",
                "The member to take the currency away from."
            ).required(true)
        )
}
