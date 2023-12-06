use crate::db::models::{ Balances, Currency };
use crate::db::uniques::{ DbChannelId, DbRoleId };
use crate::util::currency::truncate_2dp;
use anyhow::Result;
use lazy_static::lazy_static;
use rand::prelude::*;
use serenity::all::ChannelId;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::prelude::{ Channel, GuildId, Member, RoleId, UserId };
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{ debug, error, warn };

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

    let balances = Balances::try_from_user(guild_id.into(), user.into()).await?;
    let mut balances = balances.lock().await;
    let balances_ = if let Some(b) = balances.as_mut() {
        b
    } else {
        return Ok(());
    };

    let currencies = Currency::try_from_guild(guild_id.into()).await?;
    // giant for loop moment
    for curr in currencies {
        let currency = curr.read().await;
        let currency_ = if let Some(c) = currency.as_ref() {
            c
        } else {
            continue;
        };
        if !currency_.earn_by_chat() {
            continue;
        }
        let timeout_duration = currency_.earn_timeout();
        let currency_name = currency_.curr_name().to_owned();
        let earn_min = currency_.earn_min();
        let earn_max = currency_.earn_max();

        let timeout = Timeout {
            user,
            guild: guild_id,
            currency: currency_name.clone().into_string(),
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

        let balance = balances_.ensure_has_currency(currency_name.as_ref()).await?;
        // get a number between earn_min and earn_max
        let amount = truncate_2dp(rand.gen_range(earn_min..=earn_max));
        balance.add_amount(amount, None).await?;

        timeouts.insert(timeout.clone());
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
    drop(balances); // please the linter
    Ok(())
}

#[allow(clippy::useless_let_if_seq)]
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
        }
        can_earn = false;
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
        }
        can_earn = false;
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
    current_channel: impl Into<ChannelId>,
    channels: &[DbChannelId]
) -> bool {
    let id = current_channel.into();
    for db_channel in channels {
        if DbChannelId::from(id) == *db_channel {
            return true;
        }
    }
    false
}

fn check_contains_role(guild_id: GuildId, current_roles: Vec<RoleId>, roles: &[DbRoleId]) -> bool {
    let t = 0;
    for role in current_roles.iter().copied() {
        if roles.contains(&role.into()) {
            return true;
        }
    }
    false
}
