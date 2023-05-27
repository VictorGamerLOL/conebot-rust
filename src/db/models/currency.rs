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

use anyhow::{anyhow, Result};
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
            cache.put(
                (guild_id.clone(), curr_name.clone()),
                Arc::new(Mutex::new(curr)),
            );
            Some(cache.get(&(guild_id, curr_name)).unwrap().clone())
        } else {
            if cfg!(test) || cfg!(debug_assertions) {
                println!("Cache miss and not found in database!");
            }
            None
        }
    }

    /// Updates the name of the currency in the database, pops it from
    /// the cache and adds it back because the name changed.
    pub async fn update_name(self, new_name: String) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "CurrName": new_name.clone(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if the new name already exists in the guild
        let filterdoc2 = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": new_name.clone(),
        };
        if coll.find_one(filterdoc2, None).await?.is_some() {
            return Err(anyhow!(
                "Currency with name {} already exists in guild {}",
                new_name,
                self.guild_id.to_string()
            ));
        }

        let mut cache = CACHE_CURRENCY.lock().await;

        coll.update_one(filterdoc, updatedoc, None).await?;

        cache.pop(&(self.guild_id.to_string(), self.curr_name.clone()));
        cache.put(
            (self.guild_id.to_string(), new_name),
            Arc::new(Mutex::new(self.clone())),
        );
        Ok(())
    }

    /// Updates the symbol of the currency in the database.
    pub async fn update_symbol(&mut self, new_symbol: String) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "Symbol": new_symbol.clone(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.symbol = new_symbol;

        Ok(())
    }

    /// Updates whether the currency is visible to members of the guild.
    pub async fn update_visible(&mut self, new_visible: bool) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "Visible": new_visible,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.visible = new_visible;

        Ok(())
    }

    /// Updates whether the currency is the base currency of the guild.
    /// If there is already a base currency and the value passed is true,
    /// the already existing base currency will be set to false and the
    /// new base currency will be set to true.
    pub async fn update_base(&mut self, new_base: bool) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "Base": new_base,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if there is already a base currency in the guild
        // if it is, set it to false
        if new_base {
            let filterdoc2 = doc! {
                "GuildId": self.guild_id.to_string(),
                "Base": true,
            };
            let updatedoc2 = doc! {
                "$set": {
                    "Base": false,
                },
            };
            coll.update_one(filterdoc2, updatedoc2, None).await?;
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.base = new_base;

        Ok(())
    }

    /// Updates the value of the currrency in terms of the base currency.
    pub async fn update_base_value(&mut self, new_base_value: Option<f64>) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "BaseValue": new_base_value,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.base_value = new_base_value;

        Ok(())
    }

    /// Updates whether the members can pay eachother with the currency.
    pub async fn update_pay(&mut self, new_pay: bool) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "Pay": new_pay,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.pay = new_pay;

        Ok(())
    }

    /// Updates whether the members can earn the currency by chatting.
    pub async fn update_earn_by_chat(&mut self, new_earn_by_chat: bool) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "EarnByChat": new_earn_by_chat,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_by_chat = new_earn_by_chat;

        Ok(())
    }

    /// Updates whether the channels filter is in whitelist mode.
    pub async fn update_channels_is_whitelist(
        &mut self,
        new_channels_is_whitelist: bool,
    ) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "ChannelsIsWhitelist": new_channels_is_whitelist,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_is_whitelist = new_channels_is_whitelist;

        Ok(())
    }

    /// Updates whether the roles filter is in whitelist mode.
    pub async fn update_roles_is_whitelist(&mut self, new_roles_is_whitelist: bool) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "RolesIsWhitelist": new_roles_is_whitelist,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_is_whitelist = new_roles_is_whitelist;

        Ok(())
    }

    /// Adds a channel to the list of whitelisted channels.
    pub async fn add_whitelisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$push": {
                "ChannelsWhitelist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        // check if that channel is present in the whitelist
        let filterdoc2 = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsWhitelist": {
                "$in": [channel_id.to_string()],
            }
        };
        let res = coll.find_one(filterdoc2, None).await?;
        if res.is_some() {
            return Err(anyhow!("Channel already whitelisted"));
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_whitelist.push(channel_id);

        Ok(())
    }

    /// Removes a channel from the list of whitelisted channels.
    pub async fn remove_whitelisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsWhitelist": {
                "$in": [channel_id.to_string()],
            }
        };
        let updatedoc = doc! {
            "$pull": {
                "ChannelsWhitelist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_whitelist.retain(|x| x != &channel_id);

        Ok(())
    }

    /// Adds a role to the list of whitelisted roles.
    pub async fn add_whitelisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$push": {
                "RolesWhitelist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        // check if that role is present in the whitelist
        let filterdoc2 = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesWhitelist": {
                "$in": [role_id.to_string()],
            }
        };
        let res = coll.find_one(filterdoc2, None).await?;
        if res.is_some() {
            return Err(anyhow!("Role already whitelisted"));
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_whitelist.push(role_id);

        Ok(())
    }

    /// Removes a role from the list of whitelisted roles.
    pub async fn remove_whitelisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesWhitelist": {
                "$in": [role_id.to_string()],
            }
        };
        let updatedoc = doc! {
            "$pull": {
                "RolesWhitelist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_whitelist.retain(|x| x != &role_id);

        Ok(())
    }

    /// Add a channel to the list of blacklisted channels.
    pub async fn add_blacklisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$push": {
                "ChannelsBlacklist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        // check if that channel is present in the blacklist
        let filterdoc2 = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsBlacklist": {
                "$in": [channel_id.to_string()],
            }
        };
        let res = coll.find_one(filterdoc2, None).await?;
        if res.is_some() {
            return Err(anyhow!("Channel already blacklisted"));
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_blacklist.push(channel_id);

        Ok(())
    }

    /// Removes a channel from the list of blacklisted channels.
    pub async fn remove_blacklisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsBlacklist": {
                "$in": [channel_id.to_string()],
            }
        };
        let updatedoc = doc! {
            "$pull": {
                "ChannelsBlacklist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_blacklist.retain(|x| x != &channel_id);

        Ok(())
    }

    /// Adds a role to the list of blacklisted roles.
    pub async fn add_blacklisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$push": {
                "RolesBlacklist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        // check if that role is present in the blacklist
        let filterdoc2 = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesBlacklist": {
                "$in": [role_id.to_string()],
            }
        };
        let res = coll.find_one(filterdoc2, None).await?;
        if res.is_some() {
            return Err(anyhow!("Role already blacklisted"));
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_blacklist.push(role_id);

        Ok(())
    }

    /// Removes a role from the list of blacklisted roles.
    pub async fn remove_blacklisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesBlacklist": {
                "$in": [role_id.to_string()],
            }
        };
        let updatedoc = doc! {
            "$pull": {
                "RolesBlacklist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_blacklist.retain(|x| x != &role_id);

        Ok(())
    }

    /// Overwrites the entire list of whitelisted channels.
    pub async fn overwrite_whitelisted_channels(
        &mut self,
        channels: Vec<DbChannelId>,
    ) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "ChannelsWhitelist": channels.iter().map(|x| x.to_string()).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_whitelist = channels;

        Ok(())
    }

    /// Overwrites the entire list of whitelisted roles.
    pub async fn overwrite_whitelisted_roles(&mut self, roles: Vec<DbRoleId>) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "RolesWhitelist": roles.iter().map(|x| x.to_string()).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_whitelist = roles;

        Ok(())
    }

    /// Overwrites the entire list of blacklisted channels.
    pub async fn overwrite_blacklisted_channels(
        &mut self,
        channels: Vec<DbChannelId>,
    ) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "ChannelsBlacklist": channels.iter().map(|x| x.to_string()).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_blacklist = channels;

        Ok(())
    }

    /// Overwrites the entire list of blacklisted roles.
    pub async fn overwrite_blacklisted_roles(&mut self, roles: Vec<DbRoleId>) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "RolesBlacklist": roles.iter().map(|x| x.to_string()).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_blacklist = roles;

        Ok(())
    }

    /// Updates the minimum amount of currency that can be earned from a single message.
    pub async fn update_earn_min(&mut self, new_earn_min: f64) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "EarnMin": new_earn_min,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_min = new_earn_min;

        Ok(())
    }

    /// Updates the maximum amount of currency that can be earned from a single message.
    pub async fn update_earn_max(&mut self, new_earn_max: f64) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "EarnMax": new_earn_max,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_max = new_earn_max;

        Ok(())
    }

    /// Updates the amount of time (in seconds) that must pass before a user can earn currency again.
    pub async fn update_earn_timeout(&mut self, new_earn_timeout: i64) -> Result<()> {
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc = doc! {
            "$set": {
                "EarnTimeout": new_earn_timeout,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_timeout = new_earn_timeout;

        Ok(())
    }

    /// Deletes the currency from the database and removes it from the cache.
    pub async fn delete_currency(self) -> Result<()> {
        let mut cache = CACHE_CURRENCY.lock().await;

        // Delete the currency from the database.
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");
        let filterdoc = doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        coll.delete_one(filterdoc, None).await?;
        drop(db);

        // Remove the currency from the cache.
        cache.pop(&(self.guild_id.to_string(), self.curr_name));
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
