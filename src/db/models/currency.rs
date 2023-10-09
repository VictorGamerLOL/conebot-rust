//! This module contains the Currency struct and its methods.
//!
//! A currency will work in many ways since it needs to be very complex in order to accommodate for
//! the needs of many Discord servers.
//!
//! Firstly, it can work just like a normal currency bot where members have to possibility to earn it
//! `UnbelievaBoat`
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
pub mod builder;

use std::{ num::NonZeroUsize, sync::Arc };

use anyhow::{ anyhow, Result };
use chrono::Duration;
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, Collection };
use serde::{ Deserialize, Serialize };
use serde_json::{ Map, Value };
use serde_with::{ serde_as, DurationSeconds };
use serenity::model::id::ChannelId;
use serenity::model::mention::Mention;
use serenity::model::prelude::RoleId;
use thiserror::Error;
use tokio::sync::{ Mutex, RwLock };

use crate::db::id;
use crate::db::models::ToKVs;
use crate::db::{
    id::DbChannelId,
    id::DbGuildId,
    id::DbRoleId,
    ArcTokioRwLockOption,
    TokioMutexCache,
};

#[derive(Debug, Error)]
pub enum CurrencyError {
    #[error("Currency not found.")]
    NotFound,
    #[error("Currency already exists.")]
    AlreadyExists,
    #[error(transparent)] Other(#[from] anyhow::Error),
}

// Might need ^this^ later.
// I was right I did indeed need this later.

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
#[allow(clippy::struct_excessive_bools)] // If I don't put this here it will complain that im attempting to make a "state machine".
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
    #[serde_as(as = "DurationSeconds<i64>")]
    earn_timeout: Duration,
}

lazy_static! {
    // Need me that concurrency.
    static ref CACHE_CURRENCY: TokioMutexCache<(String, String), ArcTokioRwLockOption<Currency>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Currency {
    /// Attempts to fetch a Currency object from the database given a guild id and a currency name.
    ///
    /// # Errors
    /// - If no such currency exists in the database.
    /// - If any mongodb errors occur.
    ///
    /// # Examples
    /// ```rust
    /// let guild_id: u64 = 1234567890;
    /// let curr_name = "ConeCoin";
    /// let currency: Currency = Currency::try_from_name(guild_id, curr_name).await.unwrap();
    /// ```
    pub async fn try_from_name(
        guild_id: DbGuildId,
        curr_name: String
    ) -> Result<Option<ArcTokioRwLockOption<Self>>> {
        let guild_id = guild_id.to_string();

        // Try to get from cache first.
        let mut cache = CACHE_CURRENCY.lock().await;
        if let Some(currency) = cache.get(&(guild_id.clone(), curr_name.clone())) {
            return Ok(Some(currency.clone()));
        }
        // If not in cache, try to get from database. Keep holding the lock on the cache
        // so that another thread doesn't try to get the same currency from the database.
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");
        let filterdoc =
            doc! {
            "GuildId": guild_id.clone(),
            "CurrName": curr_name.clone(),
        };
        let res = coll.find_one(filterdoc, None).await?;
        drop(db); // Drop locks on mutexes as soon as possible.

        // If the currency exists, put it in the cache and return it.
        let return_val = res.map_or_else(
            || Ok(None),
            |curr| {
                let tmp = Arc::new(RwLock::new(Some(curr)));
                cache.put((guild_id.clone(), curr_name.clone()), tmp.clone());
                Ok(Some(tmp))
            }
        );
        drop(cache); // please the linter
        return_val
    }

    /// Attempts to fetch all of the currencies that a guild has made.
    ///
    /// # Errors
    /// - If any mongodb errors occur.
    pub async fn try_from_guild(guild_id: DbGuildId) -> Result<Vec<ArcTokioRwLockOption<Self>>> {
        let guild_id = guild_id.to_string();

        let mut cache = CACHE_CURRENCY.lock().await;

        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");
        let filterdoc = doc! {
            "GuildId": guild_id.clone(),
        };
        let mut res = coll.find(filterdoc, None).await?;
        drop(db); // Drop locks on mutexes as soon as possible.

        let mut currencies = Vec::new();
        while let Some(curr) = res.try_next().await? {
            let curr_name = curr.curr_name().to_owned();
            let tmp = Arc::new(RwLock::new(Some(curr)));
            currencies.push(tmp.clone());
            cache.put((guild_id.clone(), curr_name), tmp.clone());
        }
        drop(cache); // please the linter
        Ok(currencies)
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn guild_id(&self) -> &DbGuildId {
        &self.guild_id
    }

    #[allow(clippy::must_use_candidate)]
    pub fn curr_name(&self) -> &str {
        &self.curr_name
    }

    #[allow(clippy::must_use_candidate)]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn base(&self) -> bool {
        self.base
    }

    /// Literally just an alias to `base()`
    #[allow(clippy::must_use_candidate)]
    pub const fn is_base(&self) -> bool {
        self.base
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn base_value(&self) -> Option<f64> {
        self.base_value
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn pay(&self) -> bool {
        self.pay
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn earn_by_chat(&self) -> bool {
        self.earn_by_chat
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn channels_is_whitelist(&self) -> bool {
        self.channels_is_whitelist
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn roles_is_whitelist(&self) -> bool {
        self.roles_is_whitelist
    }

    #[allow(clippy::must_use_candidate)]
    pub fn channels_whitelist(&self) -> &[DbChannelId] {
        &self.channels_whitelist
    }

    #[allow(clippy::must_use_candidate)]
    pub fn roles_whitelist(&self) -> &[DbRoleId] {
        &self.roles_whitelist
    }

    #[allow(clippy::must_use_candidate)]
    pub fn channels_blacklist(&self) -> &[DbChannelId] {
        &self.channels_blacklist
    }

    #[allow(clippy::must_use_candidate)]
    pub fn roles_blacklist(&self) -> &[DbRoleId] {
        &self.roles_blacklist
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn earn_min(&self) -> f64 {
        self.earn_min
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn earn_max(&self) -> f64 {
        self.earn_max
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn earn_timeout(&self) -> Duration {
        self.earn_timeout
    }

    #[inline]
    pub fn as_base(&self, amount: f64) -> Option<f64> {
        if self.base { Some(amount) } else { self.base_value.map(|base_value| amount * base_value) }
    }

    /// Attempts to change the name of this currency.
    ///
    /// # Errors
    ///
    /// If the currency is already being used in a breaking operation, or any mongodb operation errors.
    ///
    /// # Panics
    ///
    /// It shouldn't, this is here to please the linter.
    pub async fn update_name(self_: ArcTokioRwLockOption<Self>, new_name: String) -> Result<()> {
        let mut self_ = self_.write().await;
        let mut cache = CACHE_CURRENCY.lock().await;
        // Get the cache so no other task tries to use this while it is being updated.
        if self_.is_none() {
            return Err(anyhow!("Currency is already being used in a breaking operation."));
        }
        let self__ = self_.take().unwrap(); // Safe b/c we just checked that it is not none.
        // Also all existing arcs to this should be dropped when None is seen.
        // We also still hold the lock on the mutex, so no other task can use this currency.

        let filterdoc =
            doc! {
            "GuildId": self__.guild_id.to_string(),
            "CurrName": self__.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "CurrName": new_name.clone(),
            },
        };

        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if the new name already exists in the guild
        let filterdoc2 =
            doc! {
            "GuildId": self__.guild_id.to_string(),
            "CurrName": new_name.clone(),
        };
        if coll.find_one(filterdoc2, None).await?.is_some() {
            return Err(
                anyhow!(
                    "Currency with name {} already exists in guild {}",
                    new_name,
                    self__.guild_id.to_string()
                )
            );
        }

        coll.update_one(filterdoc, updatedoc, None).await?;

        cache.pop(&(self__.guild_id.to_string(), self__.curr_name.clone()));
        cache.put(
            (self__.guild_id.to_string(), new_name),
            Arc::new(RwLock::new(Some(self__.clone())))
        );
        drop(self_); // please the linter
        drop(cache); // all hail the linter
        Ok(())
    }

    /// Updates the symbol of the currency in the database.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_symbol(&mut self, new_symbol: String) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_visible(&mut self, new_visible: bool) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_base(&mut self, new_base: bool) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "Base": new_base,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if there is already a base currency in the guild
        // if it is, set it to false
        if new_base {
            let filterdoc2 =
                doc! {
                "GuildId": self.guild_id.to_string(),
                "Base": true,
            };
            let updatedoc2 =
                doc! {
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

    /// Updates the value of the currency in terms of the base currency.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_base_value(&mut self, new_base_value: Option<f64>) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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

    /// Updates whether the members can pay each other with the currency.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_pay(&mut self, new_pay: bool) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_earn_by_chat(&mut self, new_earn_by_chat: bool) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_channels_is_whitelist(
        &mut self,
        new_channels_is_whitelist: bool
    ) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_roles_is_whitelist(&mut self, new_roles_is_whitelist: bool) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if channel is already whitelisted.
    pub async fn add_whitelisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$push": {
                "ChannelsWhitelist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if that channel is present in the whitelist
        let filterdoc2 =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if channel is not whitelisted.
    pub async fn remove_whitelisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsWhitelist": {
                "$in": [channel_id.to_string()],
            }
        };
        let updatedoc =
            doc! {
            "$pull": {
                "ChannelsWhitelist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_whitelist.retain(|x| x != &channel_id);

        Ok(())
    }

    /// Adds a role to the list of whitelisted roles.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if role is already whitelisted.
    pub async fn add_whitelisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$push": {
                "RolesWhitelist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if that role is present in the whitelist
        let filterdoc2 =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if role is not whitelisted.
    pub async fn remove_whitelisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesWhitelist": {
                "$in": [role_id.to_string()],
            }
        };
        let updatedoc =
            doc! {
            "$pull": {
                "RolesWhitelist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_whitelist.retain(|x| x != &role_id);

        Ok(())
    }

    /// Add a channel to the list of blacklisted channels.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if channel is already blacklisted.
    pub async fn add_blacklisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$push": {
                "ChannelsBlacklist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if that channel is present in the blacklist
        let filterdoc2 =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if channel is not blacklisted.
    pub async fn remove_blacklisted_channel(&mut self, channel_id: DbChannelId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "ChannelsBlacklist": {
                "$in": [channel_id.to_string()],
            }
        };
        let updatedoc =
            doc! {
            "$pull": {
                "ChannelsBlacklist": channel_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_blacklist.retain(|x| x != &channel_id);

        Ok(())
    }

    /// Adds a role to the list of blacklisted roles.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if role is already blacklisted.
    pub async fn add_blacklisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$push": {
                "RolesBlacklist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        // check if that role is present in the blacklist
        let filterdoc2 =
            doc! {
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
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors, or if role is not blacklisted.
    pub async fn remove_blacklisted_role(&mut self, role_id: DbRoleId) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
            "RolesBlacklist": {
                "$in": [role_id.to_string()],
            }
        };
        let updatedoc =
            doc! {
            "$pull": {
                "RolesBlacklist": role_id.to_string(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_blacklist.retain(|x| x != &role_id);

        Ok(())
    }

    /// Overwrites the entire list of whitelisted channels.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn overwrite_whitelisted_channels(
        &mut self,
        channels: Vec<DbChannelId>
    ) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "ChannelsWhitelist": channels.iter().map(std::string::ToString::to_string).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_whitelist = channels;

        Ok(())
    }

    /// Overwrites the entire list of whitelisted roles.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn overwrite_whitelisted_roles(&mut self, roles: Vec<DbRoleId>) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "RolesWhitelist": roles.iter().map(std::string::ToString::to_string).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_whitelist = roles;

        Ok(())
    }

    /// Overwrites the entire list of blacklisted channels.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn overwrite_blacklisted_channels(
        &mut self,
        channels: Vec<DbChannelId>
    ) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "ChannelsBlacklist": channels.iter().map(std::string::ToString::to_string).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.channels_blacklist = channels;

        Ok(())
    }

    /// Overwrites the entire list of blacklisted roles.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn overwrite_blacklisted_roles(&mut self, roles: Vec<DbRoleId>) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "RolesBlacklist": roles.iter().map(std::string::ToString::to_string).collect::<Vec<String>>(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.roles_blacklist = roles;

        Ok(())
    }

    /// Updates the minimum amount of currency that can be earned from a single message.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_earn_min(&mut self, new_earn_min: f64) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "EarnMin": new_earn_min,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_min = new_earn_min;

        Ok(())
    }

    /// Updates the maximum amount of currency that can be earned from a single message.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_earn_max(&mut self, new_earn_max: f64) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "EarnMax": new_earn_max,
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_max = new_earn_max;

        Ok(())
    }

    /// Updates the amount of time (in seconds) that must pass before a user can earn currency again.
    ///
    /// # Errors
    ///
    /// If any mongodb operation errors.
    pub async fn update_earn_timeout(&mut self, new_earn_timeout: Duration) -> Result<()> {
        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "EarnTimeout": new_earn_timeout.num_seconds(),
            },
        };
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");

        coll.update_one(filterdoc, updatedoc, None).await?;

        self.earn_timeout = new_earn_timeout;

        Ok(())
    }

    /// Consumes an Arc Mutex to a currency and deletes it from the database. Waits for
    /// all other references to the currency to be dropped before deleting.
    ///
    /// # Errors
    ///
    /// Returns an error if the currency is already being used in a breaking operation.
    ///
    /// # Panics
    ///
    /// It should not. This is here to please the linter.
    pub async fn delete_currency(self_: ArcTokioRwLockOption<Self>) -> Result<()> {
        let mut cache = CACHE_CURRENCY.lock().await; // Get the cache here so no other task
        // can get the currency while were working on it.
        let mut self_ = self_.write().await;
        let self__ = if let Some(c) = self_.take() {
            c
        } else {
            return Err(anyhow!("Currency is already being used in a breaking operation."));
        };

        // Remove the currency from the cache.
        let popped = cache.pop(&(self__.guild_id.to_string(), self__.curr_name.clone()));
        // Keep the cache past this point so that another task
        // will not try to get the currency from the db while we're deleting it.

        // Delete the currency from the database.
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("currencies");
        let filterdoc =
            doc! {
            "GuildId": self__.guild_id.to_string(),
            "CurrName": self__.curr_name.clone(),
        };
        coll.delete_one(filterdoc, None).await?;

        drop(self_); // please the linter
        drop(cache); // all hail the linter
        Ok(())
    }
}

impl ToKVs for Currency {
    fn try_to_kvs(&self) -> Result<Vec<(String, String)>> {
        match serde_json::to_value(self)? {
            Value::Object(o) =>
                Ok(
                    o
                        .into_iter()
                        .map(|(k, v)| {
                            if k == "ChannelsBlacklist" || k == "ChannelsWhitelist" {
                                // take string array, make it into json array, iterate over that,
                                // and for every value convert to str, then to string, then replace
                                // quotation marks to nothing. Then use the string obtained to make a DB
                                // channel ID, then use that to try to make a regular channel ID, then finally
                                // convert that into a mention. After that convert each mention to "Mention, ",
                                // and with the final string remove the ", " at the end.
                                let list = v
                                    .as_array()
                                    .ok_or_else(|| anyhow!("Could not convert to json array."))?
                                    .iter()
                                    .map(|v| {
                                        // this is where we take the `v` Value and convert it to a String.
                                        let val = v
                                            .as_str()
                                            .ok_or_else(||
                                                // ***12 indentation levels***
                                                anyhow!("Could not convert to json string.")
                                            )?
                                            .to_string()
                                            .replace('"', "");
                                        // Then to a DbChannelId then to a ChannelId.
                                        let db_id: DbChannelId = val.into();
                                        let id: ChannelId = db_id.try_into()?;
                                        // Then a stringified mention with leading comma and a space.
                                        Ok(format!("{}, ", Mention::from(id)))
                                    })
                                    .collect::<Result<Vec<_>>>()?;
                                Ok((
                                    k,
                                    list
                                        .into_iter()
                                        // VV merge all mentions into one string and trim the end of the last comma and space.
                                        .collect::<String>()
                                        .trim_end_matches(&[' ', ','])
                                        .to_owned(),
                                ))
                            } else if k == "RolesBlacklist" || k == "RolesWhitelist" {
                                // same here.
                                let list = v
                                    .as_array()
                                    .ok_or_else(|| anyhow!("Could not convert to json array."))?
                                    .iter()
                                    .map(|v| {
                                        let val = v
                                            .as_str()
                                            .ok_or_else(||
                                                anyhow!("Could not convert to json string.")
                                            )?
                                            .to_string()
                                            .replace('"', "");
                                        let db_id: DbRoleId = val.into();
                                        let id: RoleId = db_id.try_into()?;
                                        Ok(format!("{}, ", Mention::from(id)))
                                    })
                                    .collect::<Result<Vec<_>>>()?;
                                Ok((
                                    k,
                                    list
                                        .into_iter()
                                        .collect::<String>()
                                        .trim_end_matches(&[' ', ','])
                                        .to_owned(),
                                ))
                            } else {
                                Ok((k, v.to_string()))
                            }
                        })
                        // And finally collect all of them into a Result<Vec<(String, String)>>
                        .collect::<Result<Vec<_>>>()?
                ),
            _ => Err(anyhow!("Could not convert to json object.")),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use super::*;
    use rand::prelude::*;

    #[tokio::test]
    async fn try_from_name_test() {
        crate::init_env().await;
        let guild_id: u64 = 123_456_789; // Test currency present in the database.
        let curr_name = "test";
        let currency = Currency::try_from_name(guild_id.into(), curr_name.to_string()).await
            .unwrap()
            .unwrap();
        let currency = currency.read().await;
        let currency_ = currency.as_ref().unwrap();
        assert_eq!(currency_.guild_id, DbGuildId::from(guild_id.to_string()));
        assert_eq!(currency_.curr_name, curr_name);
        drop(currency);
    }

    // Must test multithreading
    #[tokio::test(flavor = "multi_thread")]
    async fn try_from_name_mt_staggered_test() {
        crate::init_env().await;
        let guild_id: u64 = 123_456_789;
        let curr_name = "test";
        let mut rng = thread_rng();
        let mut threads = (0..20000).map(|i|
            sleepy_fetch_currency(guild_id, curr_name, rng.gen_range(0..5000), i)
        );

        let mut handles: Vec<_> = threads.map(tokio::spawn).collect();

        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn try_from_name_mt_concurrent_test() {
        crate::init_env().await;
        let guild_id: u64 = 123_456_789;
        let curr_name = "test";
        let mut threads = (0..20000).map(|i| sleepy_fetch_currency(guild_id, curr_name, 0, i));

        let mut handles: Vec<_> = threads.map(tokio::spawn).collect();

        for h in handles {
            h.await.unwrap();
        }
    }

    async fn sleepy_fetch_currency(guild_id: u64, curr_name: &str, millis: u64, i: usize) {
        tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
        let currency = Currency::try_from_name(guild_id.into(), curr_name.to_string()).await
            .unwrap()
            .unwrap();
        let currency = currency.read().await;
        let currency_ = currency.as_ref().unwrap();
        assert_eq!(currency_.guild_id, DbGuildId::from(guild_id.to_string()));
        assert_eq!(currency_.curr_name, curr_name);
        drop(currency)
    }

    #[tokio::test]
    async fn try_create_delete() {
        crate::init_env().await;
        let mut curr = builder::Builder::new(
            DbGuildId::from(12),
            "testNo".to_string(),
            "Tt".to_string()
        );

        curr.guild_id(DbGuildId::from(123))
            .curr_name("test2".to_string())
            .symbol("T".to_string())
            .base(false)
            .base_value(Some(1.0))
            .pay(Some(true))
            .earn_by_chat(Some(true))
            .channels_is_whitelist(Some(true))
            .roles_is_whitelist(Some(true))
            .channels_whitelist(vec![DbChannelId::from(123)])
            .channels_whitelist_add(DbChannelId::from(456))
            .channels_blacklist(Some(vec![DbChannelId::from(789)]))
            .channels_blacklist_add(DbChannelId::from(101_112))
            .roles_whitelist(Some(vec![DbRoleId::from(123)]))
            .roles_whitelist_add(DbRoleId::from(456))
            .roles_blacklist(Some(vec![DbRoleId::from(789)]))
            .roles_blacklist_add(DbRoleId::from(101_112))
            .earn_min(Some(10.0))
            .earn_max(Some(100.0))
            .earn_timeout(Duration::seconds(60));
        let curr = curr.build().await.unwrap();
        for i in (0..20).rev() {
            print!("Check the db. {i} seconds left. \r");
            std::io::stdout().lock().flush().unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Currency::delete_currency(curr).await.unwrap();
    }
}
