#![allow(clippy::module_name_repetitions)] // *no*.

use std::{ borrow::Cow, num::NonZeroUsize, sync::Arc };

use anyhow::{ anyhow, bail, Result };
use futures::StreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, ClientSession, Collection };
use serde::{ Deserialize, Serialize };
use thiserror::Error;
use tokio::sync::Mutex;

use crate::db::{ uniques::{ DbGuildId, DbUserId }, ArcTokioMutexOption, TokioMutexCache };

#[derive(Debug, Clone)]
pub struct Inventory {
    guild_id: DbGuildId,
    user_id: DbUserId,
    inventory: Vec<InventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "PascalCase")]
pub struct InventoryEntry {
    guild_id: DbGuildId,
    user_id: DbUserId,
    item_name: String,
    amount: i64, // Should not go into negatives. Enforce at runtime. Here because MongoDB has no unsigned integers.
}

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("The amount of the item will underflow.")]
    AmountUnderflow,
    #[error("The amount of the item will overflow.")]
    AmountOverflow,
    #[error("The amount of the item is negative.")]
    BelowZero,
    #[error("The item does not exist or user has 0 of the item.")]
    ZeroOrNotExists,
    #[error(transparent)] Other(#[from] anyhow::Error),
}

lazy_static! {
    static ref CACHE_INVENTORY: TokioMutexCache<(DbGuildId, DbUserId), ArcTokioMutexOption<Inventory>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Inventory {
    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn user_id(&self) -> DbUserId {
        self.user_id
    }

    pub fn inventory(&self) -> &[InventoryEntry] {
        &self.inventory
    }
    /// Makes an Inventory for a user in a guild.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    pub async fn from_user(
        guild_id: DbGuildId,
        user_id: DbUserId
    ) -> Result<ArcTokioMutexOption<Self>> {
        let mut cache = CACHE_INVENTORY.lock().await;
        let key = (guild_id, user_id);
        if let Some(inventory) = cache.get(&key) {
            return Ok(inventory.to_owned());
        }
        let inv_entries = InventoryEntry::from_user(guild_id, user_id).await?;
        let inventory = Arc::new(
            Mutex::new(
                Some(Self {
                    guild_id,
                    user_id,
                    inventory: inv_entries,
                })
            )
        );
        cache.put(key, inventory.to_owned());
        drop(cache);
        Ok(inventory)
    }

    /// Gets the inventory entry matching the item name provided.
    /// If the user does not have the item or the item does not exist,
    /// it returns `None`
    ///
    /// # Errors
    /// - Item does not exist.
    /// - User has 0 of the item.
    ///
    /// # Panics
    /// Will not panic. I just use a direct vector access in there after i find
    /// the index of the item I want because there is a bug in the Rust borrow
    /// checker, and yes they are aware it exists. No they don't know how to
    /// fix it. This section is here to, once again, ***please the linter***.
    /// Whoever inspects this code and starts crying at the sight of a `vec[i]`,
    /// stop, I assure you what I did here won't crash unless `position()` fails
    /// colossally.
    pub fn get_item(&mut self, item_name: &str) -> Option<&mut InventoryEntry> {
        // There is something wrong with the borrow checker so i need to do the thing below instead.
        if let Some(i) = self.inventory.iter().position(|e| e.item_name == item_name) {
            return Some(&mut self.inventory[i]);
        }
        None
    }

    /// Gives the user the specified amount of an item. If the user has 0 of the item,
    /// it will attempt to create an inventory entry for the item.
    pub async fn give_item(
        &mut self,
        item_name: Cow<'_, str>, // Clone on write. Neat little performance improvement.
        // It more serves as a signal that "This function may or may not clone the str."
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        //                                                      VVVVVVVVVV get the &str out of the Cow<'_,str>.
        if let Some(entry) = self.get_item(&item_name) {
            entry.add_amount(amount, session).await.map_err(Into::into)
        } else {
            let entry = InventoryEntry::new(
                self.guild_id,
                self.user_id,
                // Otherwise clone it and get an entire owned string.
                item_name.into_owned(),
                amount,
                session
            ).await?;
            self.inventory.push(entry);
            Ok(())
        }
    }

    /// Takes the specified amount of an item from the user. If the user reaches 0
    /// of the item, it will delete the inventory entry for the item.
    pub async fn take_item(
        &mut self,
        item_name: &str,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        if let Some(entry) = self.get_item(item_name) {
            entry.sub_amount(
                1,
                // Since the option itself is owned, passing it would move it. Calling as_mut() on it
                // will return a Option<&mut &mut ClientSession>, but sub_amount() expects a Option<&mut ClientSession>.
                // No they are not the same thing apparently. So I need to map the &mut &mut ClientSession to &mut ClientSession
                // with the thing below by casting it. *** W O W ***.
                session.as_mut().map(|r| r as &mut ClientSession)
            ).await?;
            if entry.amount == 0 {
                self.delete_item(item_name, session).await?;
            }
            Ok(())
        } else {
            Err(anyhow!("User does not have the item or the item does not exist."))
        }
    }

    /// Gets the inventory entry matching the item and deletes it.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    /// - The item entry does not exist.
    pub async fn delete_item(
        &mut self,
        item_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<InventoryEntry> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "UserId": self.user_id.as_i64(),
            "ItemName": &item_name,
        };

        if let Some(s) = session {
            coll.delete_one_with_session(filterdoc, None, s).await?;
        } else {
            coll.delete_one(filterdoc, None).await?;
        }

        self.inventory.retain(|e| e.item_name != item_name); // slightly risky take
        // if by any chance it happens to be named differently than it is in the DB.

        Ok(())
    }

    /// Gets all of the inventories of users from a guild, then
    /// looks inside each of the inventories for the specified item. If the item
    /// is found, it will delete the inventory entry for the item. Effectively
    /// deleting the item from all users in the guild. Useful if an item is
    /// being removed from a guild.
    ///
    /// Basically a makeshift cascading delete for items.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    pub async fn purge_item(
        guild_id: DbGuildId,
        item_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        // don't forget to delete it from the cache as well
        let mut cache = CACHE_INVENTORY.lock().await;
        let mut cache_iter = cache.iter_mut();
        for (k, v) in cache_iter {
            if k.0 != guild_id {
                continue;
            }
            let mut lock_res = v.lock().await;
            if let Some(inv) = lock_res.as_mut() {
                inv.inventory.retain(|e| e.item_name != item_name);
            }
            drop(lock_res);
        }
        drop(cache);

        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<InventoryEntry> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": item_name,
        };

        if let Some(s) = session {
            coll.delete_many_with_session(filterdoc, None, s).await?;
        } else {
            coll.delete_many(filterdoc, None).await?;
        }

        Ok(())
    }
}

impl InventoryEntry {
    /// Creates a new inventory entry for an item for the specified user in the specified guild.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    /// - The item entry already exists.
    async fn new(
        guild_id: DbGuildId,
        user_id: DbUserId,
        item_name: String,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<Self> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
            "ItemName": &item_name,
        };

        if coll.find_one(filterdoc, None).await?.is_some() {
            bail!("Item already exists");
        }

        let new_self = Self {
            guild_id,
            user_id,
            item_name,
            amount,
        };

        if let Some(s) = session {
            coll.insert_one_with_session(&new_self, None, s).await?;
        } else {
            coll.insert_one(&new_self, None).await?;
        }

        Ok(new_self)
    }
    /// Fetches all of the inventory entries for a user in a guild and returns them
    /// as a vector.
    ///
    /// ***!! THE VECTOR CAN BE EMPTY IF THE USER HAS NO ITEMS !!***
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    async fn from_user(guild_id: DbGuildId, user_id: DbUserId) -> Result<Vec<Self>> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
        };

        let mut res = coll.find(filterdoc, None).await?;

        let mut items = Vec::new();

        while let Some(item) = res.next().await {
            items.push(item?);
        }
        Ok(items)
    }

    /// Fetches an inventory entry for a user in a guild and returns it.
    ///
    /// # Errors
    /// - Any mongodb error occurs, with `Err(e)`.
    /// - The item entry does not exist, with `Ok(None)`.
    async fn from_user_and_item(
        guild_id: DbGuildId,
        user_id: DbUserId,
        item_name: &str
    ) -> Result<Option<Self>> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
            "ItemName": item_name,
        };
        coll.find_one(filterdoc, None).await.map_err(|e| e.into())
    }

    /// Fetches all of the inventory entries matching the search query for a user in a guild and
    /// returns them as a vector.
    ///
    /// ***!! THE VECTOR CAN BE EMPTY IF THE USER HAS NO ITEMS MATCHING THE QUERY !!***
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    /// - Invalid regex is provided.
    async fn from_user_and_search_query(
        guild_id: DbGuildId,
        user_id: DbUserId,
        search_query: &str
    ) -> Result<Vec<Self>> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
            "ItemName": { "$regex": search_query, "$options": "i" },
        };
        let mut res = coll.find(filterdoc, None).await?;

        let mut items = Vec::new();

        while let Some(item) = res.next().await {
            items.push(item?);
        }
        Ok(items)
    }

    // I do not understand how this can be a const Fn because DbGuildId just holds a string, and
    // string borrows are not allowed in const fns.
    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn user_id(&self) -> DbUserId {
        self.user_id
    }

    pub fn item_name(&self) -> &str {
        &self.item_name
    }

    pub const fn amount(&self) -> i64 {
        self.amount
    }

    /// Sets the amount of the item in the inventory.
    ///
    /// # Errors
    /// - The amount is negative.
    /// - Any mongodb error occurs.
    pub async fn set_amount(
        &mut self,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<(), InventoryError> {
        // The negative check. The one I said I need earlier.
        if amount < 0 {
            return Err(InventoryError::BelowZero);
        }
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "UserId": self.user_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let updatedoc =
            doc! {
            "$set": {
                "Amount": amount,
            }
        };

        if let Some(s) = session {
            coll
                .update_one_with_session(filterdoc, updatedoc, None, s).await
                .map_err(|e| InventoryError::Other(e.into()))?;
        } else {
            coll
                .update_one(filterdoc, updatedoc, None).await
                .map_err(|e| InventoryError::Other(e.into()))?;
        }

        self.amount = amount;

        Ok(())
    }

    /// Subtracts the specified amount to the item in the inventory.
    ///
    /// Just an alias to `set_amount(self.amount - amount)`.
    ///
    /// # Errors
    /// - The amount is negative.
    /// - Any mongodb error occurs.
    /// - The amount underflows.
    #[inline]
    async fn sub_amount(
        &mut self,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<(), InventoryError> {
        self.set_amount(
            self.amount.checked_sub(amount).ok_or(InventoryError::AmountUnderflow)?,
            session
        ).await
    }

    /// Adds the specified amount to the item in the inventory.
    ///
    /// Just an alias to `set_amount(self.amount + amount)`.
    ///
    /// # Errors
    /// - The amount is negative.
    /// - Any mongodb error occurs.
    /// - The amount overflows.
    #[inline]
    pub async fn add_amount(
        &mut self,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<(), InventoryError> {
        self.set_amount(
            self.amount.checked_add(amount).ok_or(InventoryError::AmountOverflow)?,
            session
        ).await
    }
}
