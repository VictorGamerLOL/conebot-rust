use std::{ num::NonZeroUsize, sync::Arc };

use anyhow::{ anyhow, Result };
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, ClientSession };
use serde::{ Deserialize, Serialize };
use tokio::sync::{ Mutex, RwLock };

use crate::db::{ uniques::DbGuildId, ArcTokioRwLockOption, TokioMutexCache, CLIENT };

/// The store of a guild, formed from all the store entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Store {
    guild_id: DbGuildId,
    entries: Vec<StoreEntry>,
}

/// Represents an entry in the store of a guild.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StoreEntry {
    /// The guild this entry belongs to.
    guild_id: DbGuildId,
    /// The name of the item being sold.
    item_name: String,
    /// The name of the currency being used to buy the item.
    curr_name: String,
    /// The value of the item in the currency.
    value: f64,
    /// Amount of items you get for the price.
    amount: i64, // Should not go into negatives. Enforce at runtime.
}

lazy_static! {
    static ref CACHE_STORE: TokioMutexCache<DbGuildId, ArcTokioRwLockOption<Store>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Store {
    pub async fn try_from_guild(guild_id: DbGuildId) -> Result<ArcTokioRwLockOption<Self>> {
        let mut cache = CACHE_STORE.lock().await;
        let store = cache.get(&guild_id).map(ToOwned::to_owned);
        if let Some(store) = store {
            return Ok(store);
        }

        let self_ = Self {
            guild_id,
            entries: StoreEntry::from_guild(guild_id).await?,
        };

        let self_ = Arc::new(RwLock::new(Some(self_)));

        cache.push(guild_id, self_.clone());

        drop(cache);
        Ok(self_)
    }

    pub async fn add_entry(
        &mut self,
        item_name: String,
        curr_name: String,
        value: f64,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let entry = StoreEntry::new(
            self.guild_id,
            item_name,
            curr_name,
            value,
            amount,
            session
        ).await?;
        self.entries.push(entry);
        Ok(())
    }

    pub async fn get_entry(&self, item_name: &str, curr_name: &str) -> Option<&StoreEntry> {
        for entry in &self.entries {
            if entry.item_name == item_name && entry.curr_name == curr_name {
                return Some(entry);
            }
        }

        None
    }

    pub const fn entries(&self) -> &Vec<StoreEntry> {
        &self.entries
    }

    pub async fn edit_entry_value(
        &mut self,
        item_name: &str,
        curr_name: &str,
        value: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut index = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.item_name == item_name && entry.curr_name == curr_name {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            let entry = self.entries.get_mut(index).unwrap();
            entry.set_value(value, session).await?;
        }

        Ok(())
    }

    pub async fn edit_entry_amount(
        &mut self,
        item_name: &str,
        curr_name: &str,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut index = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.item_name == item_name && entry.curr_name == curr_name {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            let entry = self.entries.get_mut(index).unwrap();
            entry.set_amount(amount, session).await?;
        }

        Ok(())
    }

    pub async fn delete_entry(
        &mut self,
        item_name: &str,
        curr_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut index = None;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.item_name == item_name && entry.curr_name == curr_name {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            let entry = self.entries.swap_remove(index);
            entry.delete(session).await?;
        }

        Ok(())
    }
}

impl StoreEntry {
    pub async fn new(
        guild_id: DbGuildId,
        item_name: String,
        curr_name: String,
        value: f64,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<Self> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": item_name.clone(),
            "CurrName": curr_name.clone(),
        };

        if coll.find_one(filter, None).await?.is_some() {
            return Err(anyhow!("Store entry already exists."));
        }

        let entry = Self {
            guild_id,
            item_name,
            curr_name,
            value,
            amount,
        };

        if let Some(s) = session {
            coll.insert_one_with_session(entry.clone(), None, s).await?;
        } else {
            coll.insert_one(entry.clone(), None).await?;
        }

        Ok(entry)
    }

    pub async fn from_guild(guild_id: DbGuildId) -> Result<Vec<Self>> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter = doc! {
            "GuildId": guild_id.as_i64(),
        };

        let mut cursor = coll.find(filter, None).await?;

        let mut entries = Vec::new();
        while let Some(entry) = cursor.try_next().await? {
            entries.push(entry);
        }

        Ok(entries)
    }

    pub async fn from_item_and_currency_name(
        guild_id: DbGuildId,
        item_name: &str,
        curr_name: &str
    ) -> Result<Option<Self>> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": item_name,
            "CurrName": curr_name,
        };

        let mut cursor = coll.find(filter, None).await?;

        if let Some(entry) = cursor.try_next().await? {
            return Ok(Some(entry));
        }

        Ok(None)
    }

    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn item_name(&self) -> &String {
        &self.item_name
    }

    pub const fn curr_name(&self) -> &String {
        &self.curr_name
    }

    pub const fn value(&self) -> f64 {
        self.value
    }

    pub const fn amount(&self) -> i64 {
        self.amount
    }

    pub async fn set_value(
        &mut self,
        value: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": self.item_name.clone(),
            "CurrName": self.curr_name.clone(),
        };

        let update =
            doc! {
            "$set": {
                "Value": value,
            },
        };

        if let Some(s) = session {
            coll.update_one_with_session(filter, update, None, s).await?;
        } else {
            coll.update_one(filter, update, None).await?;
        }

        self.value = value;

        Ok(())
    }

    pub async fn set_amount(
        &mut self,
        amount: i64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": self.item_name.clone(),
            "CurrName": self.curr_name.clone(),
        };

        let update =
            doc! {
            "$set": {
                "Amount": amount,
            },
        };

        if let Some(s) = session {
            coll.update_one_with_session(filter, update, None, s).await?;
        } else {
            coll.update_one(filter, update, None).await?;
        }

        self.amount = amount;

        Ok(())
    }

    pub async fn delete(self, session: Option<&mut ClientSession>) -> Result<()> {
        let db = CLIENT.get().await.database("conebot");
        let coll = db.collection::<Self>("storeEntries");

        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": self.item_name,
            "CurrName": self.curr_name,
        };

        if let Some(s) = session {
            coll.delete_one_with_session(filter, None, s).await?;
        } else {
            coll.delete_one(filter, None).await?;
        }

        Ok(())
    }
}
