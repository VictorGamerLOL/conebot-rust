#![allow(clippy::module_name_repetitions)] // literally stop

pub mod builder;

use std::{ num::NonZeroUsize, sync::Arc };

use lru::LruCache;
use mongodb::bson::doc;
use serde::{ Deserialize, Serialize };
use tokio::sync::{ Mutex, RwLock };
use crate::db::{ id::{ DbGuildId, DbRoleId }, TokioMutexCache, ArcTokioRwLockOption };
use lazy_static::lazy_static;
use anyhow::{ anyhow, Result, bail };

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Item {
    guild_id: DbGuildId,
    item_name: String,
    description: String,
    sellable: bool,
    tradeable: bool,
    currency_value: String,
    value: f64,
    #[serde(flatten)]
    item_type: ItemType,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "ItemType")]
pub enum ItemType {
    #[default]
    Trophy,
    Consumable {
        #[serde(flatten)]
        action_type: ItemActionType,
    },
    InstantConsumable {
        #[serde(flatten)]
        action_type: ItemActionType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "ActionType")]
pub enum ItemActionType {
    #[default]
    None,
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))] Role {
        role_id: DbRoleId,
    },
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))] Lootbox {
        drop_table_name: String,
    },
}

lazy_static! {
    static ref CACHE_ITEM: TokioMutexCache<(DbGuildId, String), ArcTokioRwLockOption<Item>> = Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Item {
    pub async fn try_from_name(
        guild_id: DbGuildId,
        item_name: String
    ) -> anyhow::Result<ArcTokioRwLockOption<Self>> {
        let key = (guild_id.clone(), item_name.clone());
        if let Some(item) = CACHE_ITEM.lock().await.get(&key) {
            return Ok(item.clone());
        }
        let item = Self::try_from_name_uncached(guild_id, item_name).await?;
        CACHE_ITEM.lock().await.put(key, item.clone());
        Ok(item)
    }

    async fn try_from_name_uncached(
        guild_id: DbGuildId,
        item_name: String
    ) -> Result<ArcTokioRwLockOption<Self>> {
        // i am using mongodb by the way
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": guild_id.to_string(),
            "ItemName": item_name,
        };
        let item = match collection.find_one(filter, None).await {
            Ok(Some(a)) => a,
            Ok(None) => bail!("Item not found"),
            Err(e) => bail!(e),
        };
        Ok(Arc::new(RwLock::new(Some(item))))
    }

    pub fn name(&self) -> &str {
        &self.item_name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub const fn sellable(&self) -> bool {
        self.sellable
    }

    pub const fn tradeable(&self) -> bool {
        self.tradeable
    }

    pub fn currency_value(&self) -> &str {
        &self.currency_value
    }

    pub const fn value(&self) -> f64 {
        self.value
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn item_type(&self) -> &ItemType {
        &self.item_type
    }

    pub async fn update_name(self_: ArcTokioRwLockOption<Self>, new_name: String) -> Result<()> {
        let mut self_ = self_.write().await;
        let taken = self_.take(); // this must be a separate line or the linter cries abt it.
        let mut self__ = match taken {
            Some(a) => a,
            None => bail!("Item is already being used in breaking operation."),
        };
        self__.item_name = new_name;
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self__.guild_id.to_string(),
            "ItemName": self__.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "ItemName": self__.item_name.clone(),
            }
        };
        collection.update_one(filter, update, None).await?;
        let mut cache = CACHE_ITEM.lock().await;
        cache.pop(&(self__.guild_id.clone(), self__.item_name.clone()));
        cache.put(
            (self__.guild_id.clone(), self__.item_name.clone()),
            Arc::new(RwLock::new(Some(self__)))
        );
        drop(self_);
        drop(cache);
        Ok(())
    }

    pub async fn update_description(&mut self, new_description: String) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "Description": new_description.clone(),
            }
        };
        collection.update_one(filter, update, None).await?;
        self.description = new_description; // must be done at the end or it will leave undesired
        // side effects if the database update fails.
        Ok(())
    }

    pub async fn update_sellable(&mut self, new_sellable: bool) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "Sellable": new_sellable,
            }
        };
        collection.update_one(filter, update, None).await?;
        self.sellable = new_sellable;
        Ok(())
    }

    pub async fn update_tradeable(&mut self, new_tradeable: bool) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "Tradeable": new_tradeable,
            }
        };
        collection.update_one(filter, update, None).await?;
        self.tradeable = new_tradeable;
        Ok(())
    }

    pub async fn update_currency_value(&mut self, new_currency_value: String) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "CurrencyValue": new_currency_value.clone(),
            }
        };
        collection.update_one(filter, update, None).await?;
        self.currency_value = new_currency_value;
        Ok(())
    }

    pub async fn update_value(&mut self, new_value: f64) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "Value": new_value,
            }
        };
        collection.update_one(filter, update, None).await?;
        self.value = new_value;
        Ok(())
    }

    pub async fn update_message(&mut self, new_message: String) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let update =
            doc! {
            "$set": {
                "Message": new_message.clone(),
            }
        };
        collection.update_one(filter, update, None).await?;
        self.message = new_message;
        Ok(())
    }

    pub async fn update_item_type(&mut self, new_item_type: ItemType) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildID": self.guild_id.to_string(),
            "ItemName": self.item_name.clone(),
        };
        let mut update =
            doc! {
            "$unset": {
                "ActionType": "",
                "RoleId": "",
                "DropTableName": "",
            },
            "$set": {
            }
        };
        let mut update_set = update.get_document_mut("$set")?;
        // doing the thing below because bson has no idea i set serde flatten in the item struct.
        match &new_item_type {
            ItemType::Trophy => {
                update_set.insert("ActionType", "Trophy");
            }
            ItemType::Consumable { action_type } => {
                update_set.insert("ActionType", "Consumable");
                match action_type {
                    ItemActionType::None => {}
                    ItemActionType::Role { role_id } => {
                        update_set.insert("RoleId", role_id.to_string());
                    }
                    ItemActionType::Lootbox { drop_table_name } => {
                        update_set.insert("DropTableName", drop_table_name);
                    }
                }
            }
            ItemType::InstantConsumable { action_type } => {
                update_set.insert("ActionType", "InstantConsumable");
                match action_type {
                    ItemActionType::None => {}
                    ItemActionType::Role { role_id } => {
                        update_set.insert("RoleId", role_id.to_string());
                    }
                    ItemActionType::Lootbox { drop_table_name } => {
                        update_set.insert("DropTableName", drop_table_name);
                    }
                }
            }
        }
        collection.update_one(filter, update, None).await?;
        self.item_type = new_item_type;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_serialization() {
        let mut item = Item {
            item_type: ItemType::Consumable {
                action_type: ItemActionType::Role { role_id: DbRoleId::default() },
            },
            ..Default::default()
        };
        println!("{}", serde_json::to_string_pretty(&item).unwrap());
    }
}
