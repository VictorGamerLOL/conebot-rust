#![allow(clippy::module_name_repetitions)] // *no*.

use std::{ collections::HashMap, num::NonZeroUsize, sync::Arc };

use anyhow::{ anyhow, bail, Result };
use async_recursion::async_recursion;
use futures::{ StreamExt, TryStreamExt };
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, ClientSession, Collection };
use serde::{ Deserialize, Serialize };
use serenity::client::Context;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    db::{
        uniques::{ DbGuildId, DbUserId },
        ArcTokioMutexOption,
        ArcTokioRwLockOption,
        TokioMutexCache,
    },
    mechanics::item_action_handler::use_item,
};

use super::Item;

pub const INVENTORY_RECURSION_DEPTH_LIMIT: u8 = 35;

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
    pub async fn try_from_user(
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
        cache.put(key, inventory.clone());
        drop(cache);
        Ok(inventory)
    }

    /// Attempts to fetch all of the inventories of users in a guild and returns them as a vector.
    ///
    /// # Panics
    /// Will not the linter is stoopid.
    ///
    /// # Errors
    /// This function returns an error if any `MongoDB` error occurs.
    pub async fn try_from_guild(guild_id: DbGuildId) -> Result<Vec<ArcTokioMutexOption<Self>>> {
        let mut cache = CACHE_INVENTORY.lock().await;
        let mut inventory_entries = InventoryEntry::from_guild(guild_id).await?;
        let mut inventories = Vec::new();
        let entries_iter = inventory_entries.keys().copied().collect::<Vec<_>>().into_iter();
        for k in entries_iter {
            if let Some(inv) = cache.get(&(guild_id, k)) {
                inventories.push(inv.to_owned());
            } else {
                let inv = Arc::new(
                    Mutex::new(
                        Some(Self {
                            guild_id,
                            user_id: k,
                            inventory: inventory_entries.remove(&k).unwrap(),
                        })
                    )
                );
                cache.put((guild_id, k), inv.clone());
                inventories.push(inv);
            }
        }
        drop(cache);
        Ok(inventories)
    }

    /// Updates the item name in bulk for all the users in a guild.
    ///
    /// # Errors
    /// This function returns an error if any `MongoDB` error occurs.
    pub async fn bulk_update_item_name(
        guild_id: DbGuildId,
        old_name: &str,
        new_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut cache = CACHE_INVENTORY.lock().await;

        let cache_iter = cache
            .iter_mut()
            .filter(|(k, _)| k.0 == guild_id)
            .map(|(_, v)| v.to_owned())
            .collect::<Vec<_>>();

        drop(cache);

        for v in cache_iter {
            let mut lock_res = v.lock().await;
            if let Some(inv) = lock_res.as_mut() {
                for entry in &mut inv.inventory {
                    if entry.item_name == old_name {
                        entry.item_name = new_name.to_owned();
                    }
                }
            }
            drop(lock_res);
        }

        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<InventoryEntry> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": old_name,
        };
        let updatedoc =
            doc! {
            "$set": {
                "ItemName": new_name,
            }
        };

        if let Some(s) = session {
            coll
                .update_many_with_session(filterdoc, updatedoc, None, s).await
                .map_err(|e| InventoryError::Other(e.into()))?;
        } else {
            coll
                .update_many(filterdoc, updatedoc, None).await
                .map_err(|e| InventoryError::Other(e.into()))?;
        }

        Ok(())
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
    #[async_recursion]
    pub async fn give_item(
        &mut self,
        item: ArcTokioRwLockOption<Item>, // Clone on write. Neat little performance improvement.
        // It more serves as a signal that "This function may or may not clone the str."
        amount: i64,
        session: Option<&'async_recursion mut ClientSession>,
        rec_depth: u8,
        http: &Context
    ) -> Result<()> {
        if rec_depth > INVENTORY_RECURSION_DEPTH_LIMIT {
            bail!("Recursion depth exceeded.");
        }
        let item_ = item.read().await;
        let item__ = item_
            .as_ref()
            .ok_or_else(|| anyhow!("Item is being used in a breaking operation."))?;

        if item__.is_instant() {
            // DANGER don't delete this or deadlocks may occur.
            drop(item_);
            // DANGER don't delete this or deadlocks may occur.

            self.handle_instant(item.clone(), amount, rec_depth + 1, http).await?;
            return Ok(());
            // Grab hold of the lock again after it's done.
            // item_ = item.read().await;
            // item__ = item_
            //     .as_ref()
            //     .ok_or_else(|| anyhow!("Item is being used in a breaking operation."))?;
        }
        //                                                      VVVVVVVVVV get the &str out of the Cow<'_,str>.
        if let Some(entry) = self.get_item(item__.name()) {
            entry.add_amount(amount, session).await.map_err(Into::into)
        } else {
            let entry = InventoryEntry::new(
                self.guild_id,
                self.user_id,
                // Otherwise clone it and get an entire owned string.
                item__.name().to_owned(),
                amount,
                session
            ).await?;
            drop(item_);
            self.inventory.push(entry);
            Ok(())
        }
    }

    #[async_recursion]
    async fn handle_instant(
        &mut self,
        item: ArcTokioRwLockOption<Item>,
        amount: i64,
        rec_depth: u8,
        http: &Context
    ) -> Result<()> {
        use_item(self.user_id.into(), self, item, amount, rec_depth, http).await?;
        Ok(())
    }

    /// Takes the specified amount of an item from the user. If the user reaches 0
    /// of the item, it will delete the inventory entry for the item.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    pub async fn take_item(
        &mut self,
        item_name: &str,
        count: i64,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        if let Some(entry) = self.get_item(item_name) {
            if entry.amount < count {
                bail!("User does not have enough of the item.");
            }
            entry.sub_amount(
                count,
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
        let cache_iter = cache.iter_mut();
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

    async fn from_guild(guild_id: DbGuildId) -> Result<HashMap<DbUserId, Vec<Self>>> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("inventories");

        let filterdoc = doc! {
            "GuildId": guild_id.as_i64(),
        };

        let mut res = coll.find(filterdoc, None).await?;

        let mut users_with_inventories: HashMap<DbUserId, Vec<Self>> = HashMap::new();

        while let Some(inv_entry) = res.try_next().await? {
            if
                let Some(user_inventory_entries) = users_with_inventories.get_mut(
                    &inv_entry.user_id
                )
            {
                user_inventory_entries.push(inv_entry);
            } else {
                users_with_inventories.insert(inv_entry.user_id, vec![inv_entry]);
            }
        }
        Ok(users_with_inventories)
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

    pub async fn set_name(
        &mut self,
        new_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
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
                "ItemName": new_name,
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

        self.item_name = new_name.to_owned();

        Ok(())
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
    pub async fn sub_amount(
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
