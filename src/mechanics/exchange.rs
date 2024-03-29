use std::borrow::Cow;

use anyhow::{ anyhow, bail, Result };
use serenity::model::prelude::Member;
use tracing::error;

use crate::db::{ ArcTokioRwLockOption, CLIENT };
use crate::{ db::models::{ Balance, Balances, Currency }, util::currency::truncate_2dp };

/// Exchanges one currency for another.
/// Returns the amount of the output currency that was given.
///
/// # Arguments
///
/// * `input` - The currency to exchange from.
/// * `output` - The currency to exchange to.
/// * `amount` - The amount of the input currency to exchange.
/// * `user` - The user that is exchanging the currency and who's balances will be used.
///
/// # Errors
///
/// * If the input currency is not a base currency and does not have a base value.
/// * If the output currency is not a base currency and does not have a base value.
/// * If the user does not have enough of the input currency.
/// * If any of the currencies cannot be exchanged.
/// * If the exchange amount is infinite or NaN.
/// * If the exchange would lead to the user an infinite or NaN amount of the output currency.
/// * Any ``MongoDB`` errors.
pub async fn exchange(
    input: &Currency,
    output: &Currency,
    amount: f64,
    member: &Member
) -> Result<f64> {
    if input.curr_name() == output.curr_name() {
        bail!("You cannot exchange {} for itself.", input.curr_name().as_str());
    }
    let input_base_value = if let Some(f) = input.base_value() {
        f
    } else if !input.base() {
        bail!("{} cannot be exchanged.", input.curr_name().as_str());
    } else {
        1.0 // if it is a base currency, then it's base value is 1, because it is worth itself
    };
    let output_base_value = if let Some(f) = output.base_value() {
        f
    } else if !output.base() {
        bail!("{} cannot be exchanged.", output.curr_name().as_str());
    } else {
        1.0 // same here
    };

    let currencies = Currency::try_from_guild(member.guild_id.into()).await?;

    get_base_currency(currencies).await?;

    let to_give = (amount * input_base_value) / output_base_value;

    if to_give.is_infinite() || to_give.is_nan() {
        bail!("Invalid exchange rate.");
    }

    let balances = Balances::try_from_user(member.guild_id.into(), member.user.id.into()).await?;

    let mut balances = balances.lock().await;

    let balances_ = balances
        .as_mut()
        .ok_or_else(|| anyhow!("Balances are being used in a breaking operation."))?;

    balances_.ensure_has_currency(Cow::from(input.curr_name().as_str())).await?;
    balances_.ensure_has_currency(Cow::from(output.curr_name().as_str())).await?;

    let mut balance_in: Option<&mut Balance> = None; // Yes yes I know I made `ensure_has_currency` return a &mut Balance, but the thing is
    let mut balance_out: Option<&mut Balance> = None; // I can't use it twice in a row because that would mean mutably borrowing from the same place twice.
    // and I cannot do that, even if the names are different. I need to do it via the iterator below because then the borrow checker is absolutely sure
    // I have a mutable borrow from 2 different parts of the vector within `balances_`.

    for balance in balances_.balances_mut().iter_mut() {
        if balance.curr_name() == input.curr_name() {
            balance_in = Some(balance);
        } else if balance.curr_name() == output.curr_name() {
            balance_out = Some(balance);
        }
    }

    let balance_in = balance_in.ok_or_else(||
        anyhow!("No balance found for {}.", input.curr_name().as_str())
    )?;
    let balance_out = balance_out.ok_or_else(||
        anyhow!("No balance found for {}.", output.curr_name().as_str())
    )?;

    if balance_in.amount() < amount {
        bail!("You don't have enough {}.", input.curr_name().as_str());
    }

    let to_give = truncate_2dp((input_base_value * amount) / output_base_value);
    if to_give.is_infinite() || to_give.is_nan() {
        bail!("Invalid exchange rate result.");
    }

    let amount_after = balance_out.amount() + to_give;
    if amount_after.is_infinite() || amount_after.is_nan() {
        bail!("Invalid exchange rate result.");
    }

    // need the thing below to do this as a transaction.
    let mut session = CLIENT.get().await.start_session(None).await?;
    session.start_transaction(None).await?;
    if
        let Err(e) = transaction_function(
            &mut session,
            balance_in,
            amount,
            balance_out,
            to_give
        ).await
    {
        error!("Error when exchanging: {}", e);
        Balances::invalidate_cache(balances).await?; // it is important to invalidate before
        // aborting the transaction because if aborting fails, we got cache that is incorrect and that is even worse
        // than an unaborted transaction.
        session.abort_transaction().await?;
        bail!("Error when exchanging: {}", e);
    }
    session.commit_transaction().await?;

    drop(balances); // please the linter

    Ok(to_give)
}

async fn transaction_function(
    mut session: &mut mongodb::ClientSession,
    balance_in: &mut Balance,
    amount: f64,
    balance_out: &mut Balance,
    to_give: f64
) -> Result<()> {
    balance_in.sub_amount_unchecked(amount, Some(&mut session)).await?;
    balance_out.add_amount_unchecked(to_give, Some(&mut session)).await?;
    Ok(())
}

async fn get_base_currency(
    currencies: Vec<ArcTokioRwLockOption<Currency>>
) -> Result<ArcTokioRwLockOption<Currency>> {
    let mut base_currency = None;

    for currency in currencies {
        let curr = currency.read().await;
        let Some(curr_) = curr.as_ref() else {
            continue;
        };
        let is_base = curr_.base();
        drop(curr);
        if is_base {
            base_currency = Some(currency);
            break;
        }
    }

    let Some(base_currency) = base_currency else {
        bail!("No base currency found.");
    };
    Ok(base_currency)
}
