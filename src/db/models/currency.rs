//! This module contains the Currency struct and its methods.
//!
//! A currency will work in many ways since it needs to be very complex in order to accomodate for
//! the needs of many Discord servers.
//!
//! Firstly, it can work just like a normal currency bot where members have to possibility to earn it
//! by just chatting in the server, just like existing discord bots, those being UnbelievaBoat and
//! Elite Looter. They also allow you to set a custom name and custom symbols for the currency as well
//! just like them.
//!
//! Furthermore, there are the features that are a tad bit harder to implement but still present in *some*
//! bots, that being restricting where the currencies may be earned. Most bots just do this on a blacklist
//! basis, but I want to implement both a blacklist and a whitelist. This is done in order to save the amount
//! of effort of blacklisting every single required thing besides the channels/roles that are allowed to earn
//! the currency. If you toggle between blacklist and whitelist, the list for that mode will be kept and the
//! other list will be reloaded if it has been configured before.
//!
//! Additionally, also like other currency bots, there can be a configurable amount of randomness in the amount
//! and also a configurable timeout between earning currency. This is to prevent spamming and to make it more
//! fair for everyone.
mod currency_builder;

use std::{hash::Hash, num::NonZeroUsize, sync::Arc};

use anyhow::Result;
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::{DefaultHasher, LruCache};
use mongodb::{
    bson::{doc, Document, RawDocument, RawDocumentBuf},
    Collection,
};
use once_cell::sync::OnceCell;
use parking_lot::FairMutex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::db::{id::DbChannelId, id::DbGuildId, id::DbRoleId, ArcTokioMutex, TokioMutexCache};

#[derive(Debug, Clone, Error)]
pub enum CurrencyError {
    #[error("Currency not found")]
    NotFound,
    #[error("Currency already exists")]
    AlreadyExists,
    #[error("Database error")]
    DatabaseError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CurrencyUpdate {
    Name(String),
    Symbol(String),
    Visible(bool),
    Base(bool),
    BaseValue(f64),
    Pay(bool),
    EarnByChat(bool),
    ChannelsIsWhitelist(bool),
    RolesIsWhitelist(bool),
    ChannelsWhitelistAdd(DbChannelId),
    ChannelsWhitelistRemove(DbChannelId),
    ChannelsWhitelistOverwrite(Vec<DbChannelId>),
    RolesWhitelistAdd(DbRoleId),
    RolesWhitelistRemove(DbRoleId),
    RolesWhitelistOverwrite(Vec<DbRoleId>),
    ChannelsBlacklistAdd(DbChannelId),
    ChannelsBlacklistRemove(DbChannelId),
    ChannelsBlacklistOverwrite(Vec<DbChannelId>),
    RolesBlacklistAdd(DbRoleId),
    RolesBlacklistRemove(DbRoleId),
    RolesBlacklistOverwrite(Vec<DbRoleId>),
    EarnMin(f64),
    EarnMax(f64),
    EarnTimeout(i64),
}

impl ToString for CurrencyUpdate {
    fn to_string(&self) -> String {
        match self {
            CurrencyUpdate::Name(_) => "CurrName",
            CurrencyUpdate::Symbol(_) => "Symbol",
            CurrencyUpdate::Visible(_) => "Visible",
            CurrencyUpdate::Base(_) => "Base",
            CurrencyUpdate::BaseValue(_) => "BaseValue",
            CurrencyUpdate::Pay(_) => "Pay",
            CurrencyUpdate::EarnByChat(_) => "EarnByChat",
            CurrencyUpdate::ChannelsIsWhitelist(_) => "ChannelsIsWhitelist",
            CurrencyUpdate::RolesIsWhitelist(_) => "RolesIsWhitelist",
            CurrencyUpdate::ChannelsWhitelistAdd(_) => "ChannelsWhitelist",
            CurrencyUpdate::ChannelsWhitelistRemove(_) => "ChannelsWhitelist",
            CurrencyUpdate::ChannelsWhitelistOverwrite(_) => "ChannelsWhitelist",
            CurrencyUpdate::RolesWhitelistAdd(_) => "RolesWhitelist",
            CurrencyUpdate::RolesWhitelistRemove(_) => "RolesWhitelist",
            CurrencyUpdate::RolesWhitelistOverwrite(_) => "RolesWhitelist",
            CurrencyUpdate::ChannelsBlacklistAdd(_) => "ChannelsBlacklist",
            CurrencyUpdate::ChannelsBlacklistRemove(_) => "ChannelsBlacklist",
            CurrencyUpdate::ChannelsBlacklistOverwrite(_) => "ChannelsBlacklist",
            CurrencyUpdate::RolesBlacklistAdd(_) => "RolesBlacklist",
            CurrencyUpdate::RolesBlacklistRemove(_) => "RolesBlacklist",
            CurrencyUpdate::RolesBlacklistOverwrite(_) => "RolesBlacklist",
            CurrencyUpdate::EarnMin(_) => "EarnMin",
            CurrencyUpdate::EarnMax(_) => "EarnMax",
            CurrencyUpdate::EarnTimeout(_) => "EarnTimeout",
        }
        .to_string()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
/// A struct representing a currency entity in the database.
pub struct Currency {
    /// The snowflake of the guild this currency belongs to.
    /// DbGuildId is a wrapper around a string for added safety.
    /// Stored as a string because MongoDB does not support u64 and to allow the DB to be reused from other languages.
    guild_id: DbGuildId,
    /// The name of the currency.
    /// This combined with the guild_id should be unique.
    curr_name: String,
    /// The symbol of the currency.
    /// Mostly just cosmetic but must be present.
    /// Preferably a single character.
    symbol: String,
    /// Whether the currency will be visible to non-staff members.
    /// Useful for currencies that are meant to be used as a base currency.
    visible: bool,
    /// Whether this currency is used as the basis for exchange rates.
    /// Values of currencies are defined in terms of this currency.
    base: bool,
    /// How much this currency is worth in terms of the base currency.
    /// If this currency is the base currency, this value is ignored.
    base_value: Option<f64>,
    /// Whether this currency can be paid to members by other members via the pay command.
    pay: bool,
    /// Whether this currency can be earned by members via chatting in the server.
    earn_by_chat: bool,
    /// If the channels list is in whitelist mode or blacklist mode.
    channels_is_whitelist: bool,
    /// If the roles list is in whitelist mode or blacklist mode.
    roles_is_whitelist: bool,
    /// The list of channels that this currency can be earned in only (ACTIVE ONLY WITH WHITELIST).
    channels_whitelist: Vec<DbChannelId>,
    /// The list of roles that this currency can be earned by only (ACTIVE ONLY WITH WHITELIST).
    roles_whitelist: Vec<DbRoleId>,
    /// The list of channels that this currency cannot be earned in (ACTIVE ONLY WITH BLACKLIST).
    channels_blacklist: Vec<DbChannelId>,
    /// The list of roles that this currency cannot be earned by (ACTIVE ONLY WITH BLACKLIST).
    roles_blacklist: Vec<DbRoleId>,
    /// The minimum amount of currency that may be earned per message assuming earn_by_chat is true.
    earn_min: f64,
    /// The maximum amount of currency that may be earned per message assuming earn_by_chat is true.
    earn_max: f64,
    /// The amount of time in seconds that must pass before a member can earn currency again via a chat message.
    earn_timeout: i64, // Should not go into negatives, enforce at runtime.
}

lazy_static! {
    // Need me that concurrency.
    static ref CACHE_CURRENCY: TokioMutexCache<(String, String), ArcTokioMutex<Currency>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Currency {
    /// Attempts to fetch a Currency object from the database given a guild id and a currency name.
    ///
    /// # Panics
    /// If mongodb fails to execute the query.
    ///
    /// # Errors
    /// If no such currency exists in the database.
    ///
    /// # Examples
    /// ```rust
    /// let guild_id: u64 = 1234567890;
    /// let curr_name = "ConeCoin";
    /// let currency: Currency = Currency::try_from_name(guild_id, curr_name).await.unwrap();
    /// ```
    pub async fn try_from_name<T>(guild_id: T, curr_name: String) -> Option<ArcTokioMutex<Self>>
    where
        T: ToString,
    {
        let guild_id = guild_id.to_string();

        // Try to get from cache first.
        let mut cache = CACHE_CURRENCY.lock().await;
        if let Some(currency) = cache.get(&(guild_id.clone(), curr_name.clone())) {
            if cfg!(test) || cfg!(debug_assertions) {
                println!("Cache hit!");
            }
            return Some(currency.clone());
        }
        // If not in cache, try to get from database. Keep holding the lock on the cache
        // so that another thread doesn't try to get the same currency from the database.
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");
        let filterdoc = doc! {
            "GuildId": "123456789",
            "CurrName": "test",
        };
        let res = coll.find_one(filterdoc, None).await.unwrap();
        drop(db); // Drop locks on mutexes as soon as possible.

        // If the currency exists, put it in the cache and return it.
        if let Some(curr) = res {
            if cfg!(test) || cfg!(debug_assertions) {
                println!("Cache miss!");
            }
            cache.put((guild_id, curr_name), Arc::new(Mutex::new(curr.clone())));
            drop(cache);
            Some(Arc::new(Mutex::new(curr)))
        } else {
            if cfg!(test) || cfg!(debug_assertions) {
                println!("Cache miss and not found in database!");
            }
            drop(cache);
            None
        }
    }

    /// Updates the specified field of a currency in the database, making sure it will not have the same name as another currency from the same guild.
    ///
    /// # Arguments
    ///
    /// - `new_name` - The new name for the currency.
    ///
    /// # Errors
    ///
    /// If the currency with the new name already exists in the database.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let guild_id: u64 = 1234567890;
    /// let curr_name = "ConeCoin";
    /// let new_name = "NewCoin";
    /// let currency: Currency = Currency::try_from_name(guild_id, CurrencyUpdate::Name(new_name.to_string())).await.unwrap();
    /// currency.update_field(new_name.to_string()).await.unwrap();
    /// ```
    pub async fn update_field(&mut self, update_type: CurrencyUpdate) -> Result<()> {
        let mut cache = CACHE_CURRENCY.lock().await;
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");
        match update_type.clone() {
            CurrencyUpdate::ChannelsWhitelistAdd(channel_id)
            | CurrencyUpdate::ChannelsBlacklistAdd(channel_id) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$push": {
                        update_type.to_string(): channel_id.to_string(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::RolesWhitelistAdd(role_id)
            | CurrencyUpdate::RolesBlacklistAdd(role_id) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$push": {
                        update_type.to_string(): role_id.to_string(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::ChannelsWhitelistRemove(channel_id)
            | CurrencyUpdate::ChannelsBlacklistRemove(channel_id) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$pull": {
                        update_type.to_string(): channel_id.to_string(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::RolesWhitelistRemove(role_id)
            | CurrencyUpdate::RolesBlacklistRemove(role_id) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$pull": {
                        update_type.to_string(): role_id.to_string(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::ChannelsWhitelistOverwrite(channel_ids)
            | CurrencyUpdate::ChannelsBlacklistOverwrite(channel_ids) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): mongodb::bson::to_bson(&channel_ids).unwrap(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::RolesWhitelistOverwrite(roles_ids)
            | CurrencyUpdate::RolesBlacklistOverwrite(roles_ids) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): mongodb::bson::to_bson(&roles_ids).unwrap(),
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::Symbol(st) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): st,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::Base(b)
            | CurrencyUpdate::Visible(b)
            | CurrencyUpdate::Pay(b)
            | CurrencyUpdate::EarnByChat(b)
            | CurrencyUpdate::ChannelsIsWhitelist(b)
            | CurrencyUpdate::RolesIsWhitelist(b) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): b,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::EarnMin(f) | CurrencyUpdate::EarnMax(f) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): f,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::BaseValue(of) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): of,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::EarnTimeout(i) => {
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        update_type.to_string(): i,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
            CurrencyUpdate::Name(name) => {
                // check if currency with new name exists
                let res = coll
                    .find_one(
                        doc! {
                            "GuildId": self.guild_id.clone().to_string(),
                            "CurrName": name.clone(),
                        },
                        None,
                    )
                    .await?;
                if res.is_some() {
                    return Err(anyhow::anyhow!(
                        "Currency with name {} already exists",
                        name
                    ));
                }
                let filterdoc = doc! {
                    "GuildId": self.guild_id.clone().to_string(),
                    "CurrName": self.curr_name.clone(),
                };
                let updatedoc = doc! {
                    "$set": {
                        "CurrName": name,
                    },
                };
                coll.update_one(filterdoc, updatedoc, None).await?;
            }
        }
        Ok(())
    }

    pub async fn delete_currency(guild_id: String, curr_name: String) -> Result<()> {
        let mut cache = CACHE_CURRENCY.lock().await;

        // Delete the currency from the database.
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");
        let filterdoc = doc! {
            "GuildId": guild_id.clone(),
            "CurrName": curr_name.clone(),
        };
        coll.delete_one(filterdoc, None).await?;
        drop(db);

        // Remove the currency from the cache.
        cache.pop(&(guild_id.clone(), curr_name.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::Future;
    use rand::prelude::*;

    #[tokio::test]
    async fn try_from_name_test() {
        crate::init_env().await;
        let guild_id: u64 = 123456789; // Test currency present in the database.
        let curr_name = "test";
        let currency = Currency::try_from_name(guild_id, curr_name.to_string())
            .await
            .unwrap();
        let currency = currency.lock().await;
        assert_eq!(currency.guild_id, DbGuildId::from(guild_id.to_string()));
        assert_eq!(currency.curr_name, curr_name);
    }

    // Must test multithreading
    #[tokio::test(flavor = "multi_thread")]
    async fn try_from_name_mt_staggered_test() {
        crate::init_env().await;
        let guild_id: u64 = 123456789;
        let curr_name = "test";
        let mut rng = rand::thread_rng();
        let mut threads: Vec<_> = (0..20000)
            .map(|i| sleepy_fetch_currency(guild_id, curr_name, rng.gen_range(0..5000), i))
            .collect();

        let mut handles: Vec<_> = threads.into_iter().map(tokio::spawn).collect();

        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn try_from_name_mt_concurrent_test() {
        crate::init_env().await;
        let guild_id: u64 = 123456789;
        let curr_name = "test";
        let mut threads: Vec<_> = (0..20000)
            .map(|i| sleepy_fetch_currency(guild_id, curr_name, 0, i))
            .collect();

        let mut handles: Vec<_> = threads.into_iter().map(tokio::spawn).collect();

        for h in handles {
            h.await.unwrap();
        }
    }

    async fn sleepy_fetch_currency(guild_id: u64, curr_name: &str, millis: u64, i: usize) {
        tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
        println!("T{}", i);
        let currency = Currency::try_from_name(guild_id, curr_name.to_string())
            .await
            .unwrap();
        let currency = currency.lock().await;
        assert_eq!(currency.guild_id, DbGuildId::from(guild_id.to_string()));
        assert_eq!(currency.curr_name, curr_name);
    }
}
