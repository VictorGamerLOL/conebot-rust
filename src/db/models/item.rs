#![allow(clippy::module_name_repetitions)] // literally stop

pub mod builder;

use std::{ num::NonZeroUsize, sync::Arc };

use crate::db::{ uniques::{ DbGuildId, DbRoleId }, ArcTokioRwLockOption, TokioMutexCache };
use anyhow::{ anyhow, bail, Result };
use futures::StreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::bson::doc;
use mongodb::ClientSession;
use serde::{ Deserialize, Serialize };
use thiserror::Error;
use tokio::sync::{ Mutex, RwLock, RwLockWriteGuard };
use tracing::instrument;

use self::builder::{ ItemTypeTypeBuilder, ActionTypeItemTypeBuilder };

/// Represents an item a user can hold in their inventory. May or may not
/// be worth something or be used in return for something.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Item {
    /// The guild id of the guild this item belongs to.
    guild_id: DbGuildId,
    /// The name of the item, must be unique per guild.
    item_name: String,
    /// The description of the item.
    description: String,
    /// Whether the item can be sold for the currency it corresponds to.
    sellable: bool,
    /// Whether the item can be traded between users.
    tradeable: bool,
    /// The currency the item corresponds to.
    currency: String,
    /// The value of the item in the currency it corresponds to.
    value: f64,
    #[serde(flatten)]
    /// The type of the item along with its needed details.
    item_type: ItemType,
}

/// The type of the item along with its needed details.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "ItemType")]
pub enum ItemType {
    #[default]
    /// A trophy item, cannot be used as it does nothing. It just sits in your inventory.
    Trophy,
    /// A consumable item, can be used and will be removed from your inventory after use.
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
    Consumable {
        /// The message to send when the item is used.
        message: String,
        /// The action to take when the item is used, can be none. It contains the details of the
        /// action.
        #[serde(flatten)]
        action_type: ItemActionType,
    },
    /// Like consumable but it gets instantly used and removed from your inventory the moment you
    /// get it.
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
    InstantConsumable {
        /// The message to send when the item is used.
        message: String,
        /// The action to take when the item is used, can be none. It contains the details of the
        /// action.
        #[serde(flatten)]
        action_type: ItemActionType,
    },
}

impl ToString for ItemType {
    fn to_string(&self) -> String {
        match self {
            Self::Trophy => "Trophy".to_owned(),
            Self::Consumable { .. } => "Consumable".to_owned(),
            Self::InstantConsumable { .. } => "InstantConsumable".to_owned(),
        }
    }
}

/// The action to take when the item is used, can be none. It contains the details of the action.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
#[serde(tag = "ActionType")]
pub enum ItemActionType {
    /// Do nothing besides sending the message.
    #[default]
    None,
    /// Give the user a role.
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
    Role {
        /// The id of the role to give the user as a string.
        role_id: DbRoleId,
    },
    /// Opens itself as a lootbox and gives the user randomized items.
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
    Lootbox {
        /// The name of the drop table to use when opening the lootbox.
        drop_table_name: String,
    },
}

/// An error that can occur when trying to get an item from the database.
#[derive(Debug, Error)]
pub enum ItemError {
    /// The item has not been found in the database.
    #[error("The item has not been found in the database.")]
    ItemNotFound,
    /// Something else went wrong. Deal with it.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// This exists strictly for the updating of the item types via [ItemType]'s
/// `update_auto` function. It is used to make the update more intuitive on
/// the user's end.
pub enum ItemTypeUpdateType {
    Type(ItemTypeTypeBuilder),
    Message(String),
    ActionType(ActionTypeItemTypeBuilder),
    RoleId(DbRoleId),
    DropTableName(String),
}

// This giant block is needed in order to make the update more intuitive on
// the user's end. The main function responsible for this is update_auto. Which
// takes an ItemTypeUpdateType and updates the item accordingly by trying to
// figure out what the user wants to do and returning an error if it can't.
impl ItemType {
    pub fn update_auto(&self, update_type: ItemTypeUpdateType) -> Result<Self> {
        match update_type {
            ItemTypeUpdateType::Type(type_) => Ok(self.update_from_type(type_)),
            ItemTypeUpdateType::Message(message) => Ok(self.update_from_message(message)),
            ItemTypeUpdateType::ActionType(action_type) => {
                self.update_from_action_type(action_type)
            }
            ItemTypeUpdateType::RoleId(role_id) => Ok(self.update_from_role_id(role_id)),
            ItemTypeUpdateType::DropTableName(drop_table_name) => {
                Ok(self.update_from_drop_table_name(drop_table_name))
            }
        }
    }

    fn update_from_type(&self, type_: ItemTypeTypeBuilder) -> Self {
        match type_ {
            ItemTypeTypeBuilder::Trophy => Self::Trophy,
            ItemTypeTypeBuilder::Consumable => {
                Self::Consumable {
                    message: if let Self::Consumable { message, .. } = self {
                        message.clone()
                    } else if let Self::InstantConsumable { message, .. } = self {
                        message.clone()
                    } else {
                        "".to_owned()
                    },
                    action_type: if let Self::Consumable { action_type, .. } = self {
                        action_type.clone()
                    } else if let Self::InstantConsumable { action_type, .. } = self {
                        action_type.clone()
                    } else {
                        ItemActionType::None
                    },
                }
            }
            ItemTypeTypeBuilder::InstantConsumable => {
                Self::InstantConsumable {
                    message: "".to_owned(),
                    action_type: ItemActionType::None,
                }
            }
        }
    }

    fn update_from_message(&self, message: String) -> Self {
        match self {
            Self::Consumable { action_type, .. } =>
                Self::Consumable {
                    message,
                    action_type: action_type.clone(),
                },
            Self::InstantConsumable { action_type, .. } =>
                Self::InstantConsumable {
                    message,
                    action_type: action_type.clone(),
                },
            _ => Self::Consumable { message, action_type: ItemActionType::None },
        }
    }

    fn update_from_action_type(&self, action_type: ActionTypeItemTypeBuilder) -> Result<Self> {
        if action_type == ActionTypeItemTypeBuilder::Lootbox {
            bail!(
                "Cannot change to lootbox like this. Use the update_drop_table_name field instead."
            );
        } else if action_type == ActionTypeItemTypeBuilder::Role {
            bail!("Cannot change to role like this. Use the role_id field instead.");
        }
        match self {
            Self::Consumable { message, .. } =>
                Ok(Self::Consumable {
                    message: message.clone(),
                    action_type: ItemActionType::None,
                }),
            Self::InstantConsumable { message, .. } =>
                Ok(Self::InstantConsumable {
                    message: message.clone(),
                    action_type: ItemActionType::None,
                }),
            _ =>
                Ok(Self::Consumable {
                    message: "".to_owned(),
                    action_type: ItemActionType::None,
                }),
        }
    }

    fn update_from_role_id(&self, role_id: DbRoleId) -> Self {
        match self {
            Self::Consumable { message, .. } =>
                Self::Consumable {
                    message: message.clone(),
                    action_type: ItemActionType::Role { role_id },
                },
            Self::InstantConsumable { message, .. } =>
                Self::InstantConsumable {
                    message: message.clone(),
                    action_type: ItemActionType::Role { role_id },
                },
            _ =>
                Self::Consumable {
                    message: "".to_owned(),
                    action_type: ItemActionType::Role { role_id },
                },
        }
    }

    fn update_from_drop_table_name(&self, drop_table_name: String) -> Self {
        match self {
            Self::Consumable { message, .. } =>
                Self::Consumable {
                    message: message.clone(),
                    action_type: ItemActionType::Lootbox { drop_table_name },
                },
            Self::InstantConsumable { message, .. } =>
                Self::InstantConsumable {
                    message: message.clone(),
                    action_type: ItemActionType::Lootbox { drop_table_name },
                },
            _ =>
                Self::Consumable {
                    message: "".to_owned(),
                    action_type: ItemActionType::Lootbox { drop_table_name },
                },
        }
    }
}

lazy_static! {
    static ref CACHE_ITEM: TokioMutexCache<(DbGuildId, String), ArcTokioRwLockOption<Item>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Item {
    /// Attempts to get an item from the database by its name.
    ///
    /// It first checks the cache, if the item is not in the cache it will then check the database.
    ///
    /// # Errors
    /// - [`ItemError::ItemNotFound`] if the item has not been found in the database.
    /// - [`ItemError::Other`] if something else went wrong. Probably a database error.
    pub async fn try_from_name(
        guild_id: DbGuildId,
        item_name: String
    ) -> Result<ArcTokioRwLockOption<Self>, ItemError> {
        let key = (guild_id, item_name);
        if let Some(item) = CACHE_ITEM.lock().await.get(&key) {
            return Ok(item.clone());
        }
        let item = Self::try_from_name_uncached(&key.0, &key.1).await?;
        CACHE_ITEM.lock().await.put(key, item.clone());
        // were cloning a pointer above, not all of the data so it's fine.
        Ok(item)
    }

    /// Internal function to just get the item from the database without checking the cache.
    async fn try_from_name_uncached(
        guild_id: &DbGuildId,
        item_name: &str
    ) -> Result<ArcTokioRwLockOption<Self>, ItemError> {
        // i am using mongodb by the way
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": item_name,
        };
        let item = match collection.find_one(filter, None).await {
            Ok(Some(a)) => a,
            Ok(None) => {
                return Err(ItemError::ItemNotFound);
            }
            Err(e) => {
                return Err(ItemError::Other(e.into()));
            }
        };
        Ok(Arc::new(RwLock::new(Some(item))))
    }

    /// Gets all items from the database for a guild.
    pub async fn try_from_guild(
        guild_id: DbGuildId
    ) -> anyhow::Result<Vec<ArcTokioRwLockOption<Self>>> {
        // don't forget to use the cache
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter = doc! {
            "GuildId": guild_id.as_i64(),
        };
        let mut cursor = collection.find(filter, None).await?;
        let mut items = Vec::new();
        let mut cache = CACHE_ITEM.lock().await;
        while let Some(item) = cursor.next().await {
            let item = item?;
            let item_name = item.item_name.clone();
            let item_ptr = Arc::new(RwLock::new(Some(item)));
            cache.put((guild_id, item_name), item_ptr.clone());
            items.push(item_ptr);
        }
        drop(cache);
        Ok(items)
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
        &self.currency
    }

    pub const fn value(&self) -> f64 {
        self.value
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

        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self__.guild_id.as_i64(),
            "ItemName": &self__.item_name,
        };
        let update =
            doc! {
            "$set": {
                "ItemName": &new_name,
            }
        };
        collection.update_one(filter, update, None).await?;

        self__.item_name = new_name;

        let mut cache = CACHE_ITEM.lock().await;
        cache.pop(&(self__.guild_id, self__.item_name.clone()));
        cache.put((self__.guild_id, self__.item_name.clone()), Arc::new(RwLock::new(Some(self__))));

        drop(self_);
        drop(cache);
        Ok(())
    }

    pub async fn update_description(
        &mut self,
        new_description: String,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "Description": &new_description,
            }
        };
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.description = new_description; // must be done at the end or it will leave undesired
        // side effects if the database update fails.
        Ok(())
    }

    pub async fn update_sellable(
        &mut self,
        new_sellable: bool,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "Sellable": new_sellable,
            }
        };
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.sellable = new_sellable;
        Ok(())
    }

    pub async fn update_tradeable(
        &mut self,
        new_tradeable: bool,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "Tradeable": new_tradeable,
            }
        };
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.tradeable = new_tradeable;
        Ok(())
    }

    pub async fn update_currency_value(
        &mut self,
        new_currency_value: String,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "CurrencyValue": &new_currency_value,
            }
        };
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.currency = new_currency_value;
        Ok(())
    }

    pub async fn update_value(
        &mut self,
        new_value: f64,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "Value": new_value,
            }
        };
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.value = new_value;
        Ok(())
    }

    pub async fn update_item_type(
        &mut self,
        new_item_type: ItemType,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let mut update =
            doc! {
            "$unset": {
                "ActionType": "",
                "RoleId": "",
                "DropTableName": "",
                "Message": ""
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
            ItemType::Consumable { message, action_type } => {
                update_set.insert("Message", message);
                update_set.insert("ActionType", "Consumable");
                match action_type {
                    ItemActionType::None => {}
                    ItemActionType::Role { role_id } => {
                        update_set.insert("RoleId", role_id.as_i64());
                    }
                    ItemActionType::Lootbox { drop_table_name } => {
                        update_set.insert("DropTableName", drop_table_name);
                    }
                }
            }
            ItemType::InstantConsumable { message, action_type } => {
                update_set.insert("Message", message);
                update_set.insert("ActionType", "InstantConsumable");
                match action_type {
                    ItemActionType::None => {}
                    ItemActionType::Role { role_id } => {
                        update_set.insert("RoleId", role_id.as_i64());
                    }
                    ItemActionType::Lootbox { drop_table_name } => {
                        update_set.insert("DropTableName", drop_table_name);
                    }
                }
            }
        }
        if let Some(s) = session {
            collection.update_one_with_session(filter, update, None, s).await?;
        } else {
            collection.update_one(filter, update, None).await?;
        }
        self.item_type = new_item_type;
        Ok(())
    }

    // instrument at debug level with no skipping
    pub async fn delete_item(self_: ArcTokioRwLockOption<Self>) -> Result<()> {
        let mut cache = CACHE_ITEM.lock().await;
        let mut self_ = self_.write().await;
        let taken = self_.take(); // this must be a separate line or the linter cries abt it.
        let mut self__ = match taken {
            Some(a) => a,
            None => bail!("Item is already being used in breaking operation."),
        };
        let mut db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self__.guild_id.as_i64(),
            "ItemName": &self__.item_name,
        };
        collection.delete_one(filter, None).await?;
        cache.pop(&(self__.guild_id, self__.item_name.clone()));
        drop(self_);
        drop(cache);
        Ok(())
    }

    pub async fn invalidate_cache(mut self_: RwLockWriteGuard<'_, Option<Self>>) -> Result<()> {
        let mut cache = CACHE_ITEM.lock().await;
        let item = self_
            .take()
            .ok_or_else(|| anyhow!("Item is already being used in breaking operation."))?;
        cache.pop(&(item.guild_id, item.item_name));
        drop(cache);
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
                action_type: ItemActionType::Role {
                    role_id: DbRoleId::default(),
                },
                message: "A".to_string(),
            },
            ..Default::default() // Magical syntax for saying "make the rest of the fields whatever the default is."
        };
        println!("{}", serde_json::to_string_pretty(&item).unwrap());
    }
}
