use std::sync::Arc;

use anyhow::{ bail, Result, anyhow };
use serenity::model::prelude::Member;
use tokio::sync::RwLock;

use crate::{ db::models::{ Currency, Balances, Balance }, util::currency::truncate_2dp };

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
/// * Any MongoDB errors.
pub async fn exchange(
    input: &Currency,
    output: &Currency,
    amount: f64,
    user: Member
) -> Result<f64> {
    let input_base_value = if let Some(f) = input.base_value() {
        f
    } else if !input.base() {
        bail!("{} cannot be exchanged.", input.curr_name());
    } else {
        1.0 // if it is a base currency, then it's base value is 1, because it is worth itself
    };
    let output_base_value = if let Some(f) = output.base_value() {
        f
    } else if !output.base() {
        bail!("{} cannot be exchanged.", output.curr_name());
    } else {
        1.0 // same here
    };

    let mut currencies = Currency::try_from_guild(user.guild_id.into()).await?;

    get_base_currency(currencies).await?;

    let to_give = (amount * input_base_value) / output_base_value;

    if to_give.is_infinite() || to_give.is_nan() {
        bail!("Invalid exchange rate.");
    }

    let mut balances = Balances::try_from_user(user.guild_id.into(), user.user.id.into()).await?;

    let mut balances = balances.lock().await;

    let mut balances_ = balances
        .as_mut()
        .ok_or_else(|| anyhow!("Balances are being used in a breaking operation."))?;

    balances_.ensure_has_currency(input.curr_name()).await?;
    balances_.ensure_has_currency(output.curr_name()).await?;

    let mut balance_in: Option<&mut Balance> = None;
    let mut balance_out: Option<&mut Balance> = None;

    for balance in balances_.balances_mut().iter_mut() {
        if balance.curr_name() == input.curr_name() {
            balance_in = Some(balance);
        } else if balance.curr_name() == output.curr_name() {
            balance_out = Some(balance);
        }
    }

    let balance_in = balance_in.ok_or_else(||
        anyhow!("No balance found for {}.", input.curr_name())
    )?;
    let balance_out = balance_out.ok_or_else(||
        anyhow!("No balance found for {}.", output.curr_name())
    )?;

    if balance_in.amount() < amount {
        bail!("You don't have enough {}.", input.curr_name());
    }

    let to_give = truncate_2dp((input_base_value * amount) / output_base_value);
    if to_give.is_infinite() || to_give.is_nan() {
        bail!("Invalid exchange rate result.");
    }

    let mut amount_after = balance_out.amount() + to_give;
    if amount_after.is_infinite() || amount_after.is_nan() {
        bail!("Invalid exchange rate result.");
    }

    balance_in.sub_amount_unchecked(amount).await?;
    balance_out.add_amount_unchecked(to_give).await?;
    drop(balances); // please the linter

    Ok(to_give)
}

async fn get_base_currency(
    currencies: Vec<Arc<RwLock<Option<Currency>>>>
) -> Result<Arc<RwLock<Option<Currency>>>> {
    let mut base_currency = None;

    for currency in currencies {
        let curr = currency.read().await;
        let curr_ = if let Some(c) = curr.as_ref() {
            c
        } else {
            continue;
        };
        let is_base = curr_.base();
        drop(curr);
        if is_base {
            base_currency = Some(currency);
            break;
        }
    }

    let mut base_currency = if let Some(c) = base_currency {
        c
    } else {
        bail!("No base currency found.");
    };
    Ok(base_currency)
}
