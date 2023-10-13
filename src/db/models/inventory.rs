#![allow(clippy::module_name_repetitions)] // *no*.

use std::{ num::NonZeroUsize, sync::Arc };

use anyhow::{ anyhow, bail, Result };
use futures::{ StreamExt, TryStreamExt };
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, Collection };
use serde::{ Deserialize, Serialize };
use tokio::sync::Mutex;

use crate::db::{ id::{ DbGuildId, DbUserId }, ArcTokioMutexOption, TokioMutexCache };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    guild_id: DbGuildId,
    user_id: DbUserId,
    inventory: Vec<InventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEntry {
    guild_id: DbGuildId,
    user_id: DbUserId,
    item_name: String,
    amount: i64, // Should not go into negatives. Enforce at runtime. Here because MongoDB has no unsigned integers.
}

lazy_static! {
    static ref CACHE_INVENTORY: TokioMutexCache<(DbGuildId, DbUserId), ArcTokioMutexOption<Inventory>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Inventory {
    /// Makes an Inventory for a user in a guild.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    pub async fn from_user(
        guild_id: DbGuildId,
        user_id: DbUserId
    ) -> Result<ArcTokioMutexOption<Self>> {
        let mut cache = CACHE_INVENTORY.lock().await;
        let key = (guild_id.clone(), user_id.clone());
        if let Some(inventory) = cache.get(&key) {
            return Ok(inventory.clone());
        }
        let inv_entries = InventoryEntry::from_user(&guild_id, &user_id).await?;
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

    /// Gets the inventory entry matching the item name provided,
    /// or makes it.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    ///
    /// # Panics
    /// Will not panic. I just use a direct vector access in there after i find
    /// the index of the item I want because there is a bug in the Rust borrow
    /// checker, and yes they are aware it exists. No they don't know how to
    /// fix it. This section is here to, once again, ***please the linter***.
    /// Whoever inspects this code and starts crying at the sight of a `vec[i]`,
    /// stop, I assure you what I did here won't crash unless `position()` fails
    /// colossally.
    pub async fn get_or_make_item(&mut self, item_name: String) -> Result<&mut InventoryEntry> {
        // There is something wrong with the borrow checker so i need to do the thing below instead.
        if let Some(i) = self.inventory.iter().position(|e| e.item_name == item_name) {
            return Ok(&mut self.inventory[i]);
        }

        let entry = InventoryEntry::new(&self.guild_id, &self.user_id, item_name).await?;
        self.inventory.push(entry);
        Ok(self.inventory.last_mut().unwrap())
    }

    /// Gets the inventory entry matching the item and deletes it.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    /// - The item entry does not exist.
    pub async fn delete_item(&mut self, item_name: String) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<InventoryEntry> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "UserId": self.user_id.to_string(),
            "ItemName": &item_name,
        };

        coll.delete_one(filterdoc, None).await?;

        self.inventory.retain(|e| e.item_name != item_name); // slightly risky take
        // if by any chance it happens to be named differently than it is in the DB.

        Ok(())
    }
}

impl InventoryEntry {
    /// Creates a new inventory entry for an item for the specified user in the specified guild.
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    /// - The item entry already exists.
    async fn new(guild_id: &DbGuildId, user_id: &DbUserId, item_name: String) -> Result<Self> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
            "ItemName": item_name.clone(),
        };

        if coll.find_one(filterdoc, None).await?.is_some() {
            bail!("Item already exists");
        }

        let new_self = Self {
            guild_id: guild_id.clone(),
            user_id: user_id.clone(),
            item_name,
            amount: 0,
        };

        coll.insert_one(&new_self, None).await?;

        Ok(new_self)
    }
    /// Fetches all of the inventory entries for a user in a guild and returns them
    /// as a vector.
    ///
    /// ***!! THE VECTOR CAN BE EMPTY IF THE USER HAS NO ITEMS !!***
    ///
    /// # Errors
    /// - Any mongodb error occurs.
    async fn from_user(guild_id: &DbGuildId, user_id: &DbUserId) -> Result<Vec<Self>> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
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
        item_name: String
    ) -> Result<Option<Self>> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<Self> = db.collection("inventories");

        let mut filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
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
        search_query: String
    ) -> Result<Vec<Self>> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<Self> = db.collection("inventories");

        let mut filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
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
    pub const fn guild_id(&self) -> &DbGuildId {
        &self.guild_id
    }

    pub const fn user_id(&self) -> &DbUserId {
        &self.user_id
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
    pub async fn set_amount(&mut self, amount: i64) -> Result<()> {
        if amount < 0 {
            bail!("Amount cannot be negative");
        }
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let mut coll: Collection<Self> = db.collection("inventories");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "UserId": self.user_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "Amount": amount,
            }
        };

        coll.update_one(filterdoc, updatedoc, None).await?;

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
    pub async fn sub_amount(&mut self, amount: i64) -> Result<()> {
        self.set_amount(
            self.amount.checked_sub(amount).ok_or_else(|| anyhow!("Amount will underflow"))?
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
    pub async fn add_amount(&mut self, amount: i64) -> Result<()> {
        self.set_amount(
            self.amount.checked_add(amount).ok_or_else(|| anyhow!("Amount will overflow"))?
        ).await
    }
}
