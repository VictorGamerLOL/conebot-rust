use crate::db::id::{ DbGuildId, DbUserId, DbChannelId, DbRoleId };
use crate::db::models::{ Currency, Balance, Balances };
use crate::util::currency::truncate_2dp;
use anyhow::Result;
use lazy_static::lazy_static;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::prelude::{ UserId, GuildId, Member, Channel, RoleId };
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{ info, warn, error, debug };
use rand::prelude::*;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Timeout {
    pub user: UserId,
    pub guild: GuildId,
    pub currency: String,
}

lazy_static! {
    static ref TIMEOUTS: Arc<Mutex<HashSet<Timeout>>> = Arc::new(Mutex::new(HashSet::new()));
}

pub async fn message(_ctx: Context, new_message: Message) -> Result<()> {
    debug!("Got message: {:?}", new_message);
    if new_message.author.bot {
        return Ok(());
    }
    let mut rand = rand::rngs::OsRng;
    let user: UserId = new_message.author.id; // this instead of Db<Whatever> because these implement `Copy`.
    let guild_id: GuildId = if let Some(g) = new_message.guild_id {
        g
    } else {
        return Ok(());
    };

    let mut balances = Balances::try_from_user(guild_id.into(), user.into()).await?;
    let mut balances = balances.lock().await;
    let mut balances_ = if let Some(b) = balances.as_mut() {
        b
    } else {
        return Ok(());
    };

    let mut currencies = Currency::try_from_guild(guild_id.into()).await?;
    for curr in currencies {
        let mut currency = curr.lock().await;
        let currency_ = if let Some(c) = currency.as_ref() {
            c
        } else {
            continue;
        };
        let timeout_duration = currency_.earn_timeout();
        let currency_name = currency_.curr_name().to_owned();
        let earn_min = currency_.earn_min();
        let earn_max = currency_.earn_max();

        let timeout = Timeout {
            user,
            guild: guild_id,
            currency: currency_name.clone(),
        };

        let mut timeouts = TIMEOUTS.lock().await;

        if timeouts.contains(&timeout) {
            continue;
        }

        let member = match new_message.member(&_ctx.http).await {
            Ok(m) => m,
            Err(e) => {
                warn!("Failed to get member: {}", e);
                continue;
            }
        };

        let channel = match new_message.channel(&_ctx.http).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to get channel: {}", e);
                continue;
            }
        };

        if !check_can_earn(guild_id, member.clone(), channel.clone(), currency_) {
            continue;
        }

        drop(currency);

        balances_.ensure_has_currency(&currency_name).await?;
        let mut balance = if
            let Some(b) = balances_
                .balances_mut()
                .iter_mut()
                .find(|b| b.curr_name == currency_name)
        {
            b
        } else {
            warn!("Failed to get balance for currency {}", currency_name);
            continue;
        };
        // get a number between earn_min and earn_max
        let amount = truncate_2dp(rand.gen_range(earn_min..=earn_max));
        balance.add_amount(amount).await?;

        timeouts.insert(dbg!(timeout.clone()));
        drop(timeouts);

        tokio::spawn(async move {
            let std_duration = if let Ok(timeout_duration) = timeout_duration.to_std() {
                timeout_duration
            } else {
                error!("Failed to convert chrono duration {:?} to std duration", timeout_duration);
                return;
            };
            debug!("Sleeping for {:?}", std_duration);
            tokio::time::sleep(std_duration).await;
            debug!("Done sleeping for {:?}", std_duration);
            let mut timeouts = TIMEOUTS.lock().await;
            timeouts.remove(&timeout);
            drop(timeouts);
        });
    }
    Ok(())
}

fn check_can_earn(
    guild_id: GuildId,
    member: Member,
    channel: Channel,
    currency: &Currency
) -> bool {
    let mut can_earn = true;
    if currency.roles_is_whitelist() {
        let roles = currency.roles_whitelist();
        if check_contains_role(guild_id, member.roles, roles) {
            return true;
        } else {
            can_earn = false;
        }
    } else {
        let roles = currency.roles_blacklist();
        if check_contains_role(guild_id, member.roles, roles) {
            return false;
        }
    }
    if currency.channels_is_whitelist() {
        let channels = currency.channels_whitelist();
        if check_contains_channel(guild_id, channel, channels) {
            return true;
        } else {
            can_earn = false;
        }
    } else {
        let channels = currency.channels_blacklist();
        if check_contains_channel(guild_id, channel, channels) {
            return false;
        }
    }
    can_earn
}

fn check_contains_channel(
    guild_id: GuildId,
    current_channel: Channel,
    channels: &[DbChannelId]
) -> bool {
    for db_channel in channels {
        if current_channel.id().0.to_string() == db_channel.0 {
            return true;
        } else {
            continue;
        }
    }
    false
}

fn check_contains_role(guild_id: GuildId, current_roles: Vec<RoleId>, roles: &[DbRoleId]) -> bool {
    for role in current_roles {
        if roles.contains(&crate::db::id::DbRoleId(role.0.to_string())) {
            return true;
        } else {
            continue;
        }
    }
    false
}
