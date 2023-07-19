use anyhow::{ anyhow, Result };
use serenity::{
    builder::CreateApplicationCommand,
    http::{ Http, CacheHttp },
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction,
                CommandDataOption,
                CommandDataOptionValue,
            },
            Member,
            PartialMember,
            command::CommandOptionType,
        },
        user::User,
        Permissions,
    },
};

use crate::{ util::currency::truncate_2dp, db::models::{ Balances, Currency } };

/// # Errors
/// TODO
///
/// # Panics
/// will not
#[allow(clippy::unused_async)]
#[allow(clippy::cast_precision_loss)]
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let mut amount: f64 = 0.0;
    let mut currency = String::new();
    let mut member: User = User::default();

    let options: Vec<(String, CommandDataOptionValue)> = options
        .iter()
        .cloned()
        .map(
            |o| -> Result<(String, CommandDataOptionValue)> {
                let res = o.resolved.ok_or_else(|| anyhow!("Failed to resolve {}", o.name))?;
                Ok((o.name, res))
            }
        )
        .collect::<Result<Vec<(String, CommandDataOptionValue)>>>()?;

    for (name, option) in options {
        if name == *"currency" {
            currency = if let CommandDataOptionValue::String(s) = option {
                s
            } else {
                return Err(anyhow!("Did not find a string for currency name."));
            };
        } else if name == *"amount" {
            amount = if let CommandDataOptionValue::Number(f) = option {
                f
            } else if let CommandDataOptionValue::Integer(i) = option {
                i as f64
            } else {
                return Err(anyhow!("Did not find a number or integer for amount."));
            };
        } else if name == *"member" {
            member = if let CommandDataOptionValue::User(u, _) = option {
                u
            } else {
                return Err(anyhow!("Did not find a user for member."));
            };
        }
    }

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
