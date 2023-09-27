use anyhow::{ bail, Result };
use serenity::model::prelude::Member;

use crate::db::models::{ Currency, Balances };

pub async fn exchange(
    input: &Currency,
    output: &Currency,
    amount: f64,
    user: Member
) -> Result<bool> {
    if input.base_value().is_none() || output.base_value().is_none() {
        bail!("One or both of the currencies cannot be exchanged.");
    }
    let mut currencies = Currency::try_from_guild(user.guild_id.into()).await?;
    //TODO
    let mut balances = Balances::try_from_user(user.guild_id.into(), user.user.id.into()).await?;

    Ok(true)
}
