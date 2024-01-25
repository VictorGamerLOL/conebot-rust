#![allow(clippy::module_name_repetitions)] // literally stop
#![allow(clippy::must_use_candidate)]

pub mod builder;
pub mod fieldless;
pub mod name_updates_handler;

use std::{ num::NonZeroUsize, sync::Arc };

use self::{ fieldless::ItemActionTypeFieldless, name_updates_handler::handle_name_updates };
use crate::db::{
    uniques::{ DbGuildId, DbRoleId, DropTableName, DropTableNameRef },
    ArcTokioRwLockOption,
    TokioMutexCache,
    CLIENT,
};
use anyhow::{ anyhow, bail, Result };
use fieldless::ItemTypeFieldless;
use futures::StreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::bson::doc;
use mongodb::ClientSession;
use serde::{ Deserialize, Serialize };
use thiserror::Error;
use tokio::sync::{ Mutex, RwLock, RwLockWriteGuard };

use super::ToKVs;

/// Represents an item a user can hold in their inventory. May or may not
/// be worth something or be used in return for something.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, PartialOrd)]
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
    // Serde flatten is here so the item does not end up like
    /*
    {
        ...
        item_type: {...}
    }
     */
    // and instead it ends up.. well.. flat
    /*
    {
        ...
        item_type: "Consumable",
        ...
    }
     */
    #[serde(flatten)]
    /// The type of the item along with its needed details.
    item_type: ItemType,
}

/// The type of the item along with its needed details.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "ItemType")]
#[non_exhaustive]
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
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
        count: i64,
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

/// This exists strictly for the updating of the item types via ``[ItemType]``'s
/// `update_auto` function. It is used to make the update more intuitive on
/// the user's end.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ItemTypeUpdateType {
    Type(ItemTypeFieldless),
    Message(String),
    ActionType(ItemActionTypeFieldless),
    RoleId(DbRoleId),
    DropTableName(DropTableName),
    Count(i64),
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
            ItemTypeUpdateType::Count(count) => Ok(self.update_from_count(count)?),
        }
    }

    fn update_from_type(&self, type_: ItemTypeFieldless) -> Self {
        match type_ {
            ItemTypeFieldless::Trophy => Self::Trophy,
            ItemTypeFieldless::Consumable =>
                Self::Consumable {
                    message: if let Self::Consumable { message, .. } = self {
                        message.clone()
                    } else if let Self::InstantConsumable { message, .. } = self {
                        message.clone()
                    } else {
                        String::new()
                    },
                    action_type: if let Self::Consumable { action_type, .. } = self {
                        action_type.clone()
                    } else if let Self::InstantConsumable { action_type, .. } = self {
                        action_type.clone()
                    } else {
                        ItemActionType::None
                    },
                },
            ItemTypeFieldless::InstantConsumable =>
                Self::InstantConsumable {
                    message: String::new(),
                    action_type: ItemActionType::None,
                },
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
            _ =>
                Self::Consumable {
                    message,
                    action_type: ItemActionType::None,
                },
        }
    }

    #[allow(unreachable_patterns)] // I said the enum is non_exhaustive why do I need to do this.
    fn update_from_action_type(&self, action_type: ItemActionTypeFieldless) -> Result<Self> {
        if action_type == ItemActionTypeFieldless::Lootbox {
            bail!(
                "Cannot change to lootbox like this. Use the update_drop_table_name field instead."
            );
        } else if action_type == ItemActionTypeFieldless::Role {
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
            Self::Trophy =>
                Ok(Self::Consumable {
                    message: String::new(),
                    action_type: ItemActionType::None,
                }),
            _ => { bail!("Unimplemented.") }
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
                    message: String::new(),
                    action_type: ItemActionType::Role { role_id },
                },
        }
    }

    fn update_from_drop_table_name(&self, drop_table_name: DropTableName) -> Self {
        let count = self.count().unwrap_or(1);
        match self {
            Self::Consumable { message, .. } =>
                Self::Consumable {
                    message: message.clone(),
                    action_type: ItemActionType::Lootbox {
                        drop_table_name: drop_table_name.into_string(),
                        count,
                    },
                },
            Self::InstantConsumable { message, .. } =>
                Self::InstantConsumable {
                    message: message.clone(),
                    action_type: ItemActionType::Lootbox {
                        drop_table_name: drop_table_name.into_string(),
                        count,
                    },
                },
            _ =>
                Self::Consumable {
                    message: String::new(),
                    action_type: ItemActionType::Lootbox {
                        drop_table_name: drop_table_name.into_string(),
                        count,
                    },
                },
        }
    }

    fn update_from_count(&self, count: i64) -> Result<Self> {
        match self {
            Self::Consumable { message, action_type } =>
                Ok(Self::Consumable {
                    message: message.clone(),
                    action_type: match action_type {
                        ItemActionType::Lootbox { drop_table_name, .. } =>
                            ItemActionType::Lootbox {
                                drop_table_name: drop_table_name.clone(),
                                count,
                            },
                        _ => bail!("Cannot update count on non-lootbox item."),
                    },
                }),
            Self::InstantConsumable { message, action_type } =>
                Ok(Self::InstantConsumable {
                    message: message.clone(),
                    action_type: match action_type {
                        ItemActionType::Lootbox { drop_table_name, .. } =>
                            ItemActionType::Lootbox {
                                drop_table_name: drop_table_name.clone(),
                                count,
                            },
                        _ => bail!("Cannot update count on non-lootbox item."),
                    },
                }),
            _ => bail!("Cannot update count on non-lootbox item."),
        }
    }

    pub const fn message(&self) -> Option<&String> {
        match self {
            Self::Consumable { message, .. } => Some(message),
            Self::InstantConsumable { message, .. } => Some(message),
            _ => None,
        }
    }

    pub const fn action_type(&self) -> Option<&ItemActionType> {
        match self {
            Self::Consumable { action_type, .. } => Some(action_type),
            Self::InstantConsumable { action_type, .. } => Some(action_type),
            _ => None,
        }
    }

    pub const fn role_id(&self) -> Option<DbRoleId> {
        match self {
            Self::Consumable { action_type: ItemActionType::Role { role_id }, .. } =>
                Some(*role_id),
            Self::InstantConsumable { action_type: ItemActionType::Role { role_id }, .. } =>
                Some(*role_id),
            _ => None,
        }
    }

    pub const fn drop_table_name_string(&self) -> Option<&String> {
        match self {
            Self::Consumable { action_type: ItemActionType::Lootbox { drop_table_name, .. }, .. } =>
                Some(drop_table_name),
            Self::InstantConsumable {
                action_type: ItemActionType::Lootbox { drop_table_name, .. },
                ..
            } => Some(drop_table_name),
            _ => None,
        }
    }

    pub const fn count(&self) -> Option<i64> {
        match self {
            Self::Consumable { action_type: ItemActionType::Lootbox { count, .. }, .. } =>
                Some(*count),
            Self::InstantConsumable { action_type: ItemActionType::Lootbox { count, .. }, .. } =>
                Some(*count),
            _ => None,
        }
    }

    pub const fn to_fieldless(&self) -> ItemTypeFieldless {
        match self {
            Self::Trophy => ItemTypeFieldless::Trophy,
            Self::Consumable { .. } => ItemTypeFieldless::Consumable,
            Self::InstantConsumable { .. } => ItemTypeFieldless::InstantConsumable,
        }
    }
}

impl ItemActionType {
    pub const fn to_fieldless(&self) -> ItemActionTypeFieldless {
        match self {
            Self::None => ItemActionTypeFieldless::None,
            Self::Role { .. } => ItemActionTypeFieldless::Role,
            Self::Lootbox { .. } => ItemActionTypeFieldless::Lootbox,
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
        let item = Self::try_from_name_uncached(key.0, &key.1).await?;
        CACHE_ITEM.lock().await.put(key, item.clone());
        // were cloning a pointer above, not all of the data so it's fine.
        Ok(item)
    }

    /// Internal function to just get the item from the database without checking the cache.
    async fn try_from_name_uncached(
        guild_id: DbGuildId,
        item_name: &str
    ) -> Result<ArcTokioRwLockOption<Self>, ItemError> {
        // i am using mongodb by the way
        let db = crate::db::CLIENT.get().await.database("conebot");
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
        let db = crate::db::CLIENT.get().await.database("conebot");
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

    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
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

    pub const fn message(&self) -> Option<&String> {
        match self.item_type {
            | ItemType::InstantConsumable { ref message, .. }
            | ItemType::Consumable { ref message, .. } => Some(message),
            _ => None,
        }
    }

    pub const fn action_type(&self) -> Option<&ItemActionType> {
        match self.item_type {
            | ItemType::InstantConsumable { ref action_type, .. }
            | ItemType::Consumable { ref action_type, .. } => Some(action_type),
            _ => None,
        }
    }

    pub const fn drop_table_name(&self) -> Option<DropTableNameRef<'_>> {
        match self.item_type {
            | ItemType::InstantConsumable {
                  action_type: ItemActionType::Lootbox { ref drop_table_name, .. },
                  ..
              }
            | ItemType::Consumable {
                  action_type: ItemActionType::Lootbox { ref drop_table_name, .. },
                  ..
              } =>
                Some(unsafe {
                    DropTableNameRef::from_string_ref_and_guild_id_unchecked(
                        self.guild_id,
                        drop_table_name
                    )
                }),
            _ => None,
        }
    }

    pub const fn role_id(&self) -> Option<DbRoleId> {
        match self.item_type {
            | ItemType::InstantConsumable { action_type: ItemActionType::Role { role_id }, .. }
            | ItemType::Consumable { action_type: ItemActionType::Role { role_id }, .. } =>
                Some(role_id),
            _ => None,
        }
    }

    pub const fn is_instant(&self) -> bool {
        matches!(self.item_type, ItemType::InstantConsumable { .. })
    }

    pub const fn is_trophy(&self) -> bool {
        matches!(self.item_type, ItemType::Trophy)
    }

    /// Updates the name of the item. Since name updates are a sensitive operation,
    /// this function instead takes an Arc, and also tries to lock several other things
    /// in order to ensure everything is updated. ***MUST*** not have locks of the following
    /// before calling this in the same function:
    /// - [`crate::db::models::inventory::Inventory`]
    /// - [`crate::db::models::drop_table::DropTable`]
    /// - [`crate::db::models::store_entry::StoreEntry`]
    /// - The item itself.
    pub async fn update_name(
        self_: ArcTokioRwLockOption<Self>,
        new_name: String,
        mut session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut self_ = self_.write().await;
        let taken = self_.take(); // this must be a separate line or the linter cries abt it.
        let mut self__ = match taken {
            Some(a) => a,
            None => bail!("Item is already being used in breaking operation."),
        };

        let db = crate::db::CLIENT.get().await.database("conebot");
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

        let mut maybe_session: Option<ClientSession>;

        if session.is_none() {
            maybe_session = Some(CLIENT.get().await.start_session(None).await?);
            session = maybe_session.as_mut();
        }

        let session = session.unwrap();

        collection.update_one_with_session(filter, update, None, session).await?;

        handle_name_updates(
            self__.guild_id,
            self__.item_name.clone(),
            new_name.clone(),
            session
        ).await?;

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
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
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
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
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
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
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
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update =
            doc! {
            "$set": {
                "Currency": &new_currency_value,
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
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let db = crate::db::CLIENT.get().await.database("conebot");
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
        // TODO: make a smarter update function that only updates the fields that have changed.
        // So it does not need to do 2 updates.
        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name,
        };
        let update_unset =
            doc! {
            "$unset": {
                "ItemType": "",
                "ActionType": "",
                "RoleId": "",
                "DropTableName": "",
                "Message": "",
                "Count": ""
            },
        };
        if let Some(ref mut s) = session {
            collection.update_one_with_session(filter.clone(), update_unset, None, s).await?;
        } else {
            collection.update_one(filter.clone(), update_unset, None).await?;
        }
        let mut update = doc! {
            "$set": {
            }
        };
        let update_set = update.get_document_mut("$set")?;
        // doing the thing below because bson has no idea i set serde flatten in the item struct.
        match &new_item_type {
            ItemType::Trophy => {
                update_set.insert("ItemType", "Trophy");
            }
            ItemType::Consumable { message, action_type } => {
                update_set.insert("Message", message);
                update_set.insert("ItemType", "Consumable");

                match action_type {
                    ItemActionType::None => {
                        update_set.insert("ActionType", "None");
                    }
                    ItemActionType::Role { role_id } => {
                        update_set.insert("ActionType", "Role");
                        update_set.insert("RoleId", role_id.as_i64());
                    }
                    ItemActionType::Lootbox { drop_table_name, count } => {
                        update_set.insert("ActionType", "Lootbox");
                        update_set.insert("DropTableName", drop_table_name);
                        update_set.insert("Count", count);
                    }
                }
            }
            ItemType::InstantConsumable { message, action_type } => {
                update_set.insert("Message", message);
                update_set.insert("ItemType", "InstantConsumable");
                match action_type {
                    ItemActionType::None => {
                        update_set.insert("ActionType", "None");
                    }
                    ItemActionType::Role { role_id } => {
                        update_set.insert("ActionType", "Role");
                        update_set.insert("RoleId", role_id.as_i64());
                    }
                    ItemActionType::Lootbox { drop_table_name, count } => {
                        update_set.insert("ActionType", "Lootbox");
                        update_set.insert("DropTableName", drop_table_name);
                        update_set.insert("Count", count);
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

    pub async fn bulk_update_currency_value_name(
        guild_id: DbGuildId,
        before: &str,
        after: String,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut cache = CACHE_ITEM.lock().await;

        let rw_locks = cache
            .iter_mut()
            .filter_map(|(k, v)| {
                if k.0 == guild_id { Some(v.to_owned()) } else { None }
            })
            .collect::<Vec<_>>();

        drop(cache);

        for rw_lock in rw_locks {
            let mut lock = rw_lock.write().await;
            if let Some(item) = lock.as_mut() {
                if item.currency == before {
                    item.currency = after.clone();
                }
            }
        }

        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "Currency": before,
        };
        let update =
            doc! {
            "$set": {
                "Currency": &after,
            }
        };

        if let Some(s) = session {
            collection.update_many_with_session(filter, update, None, s).await?;
        } else {
            collection.update_many(filter, update, None).await?;
        }

        Ok(())
    }

    pub async fn delete_item(
        self_: ArcTokioRwLockOption<Self>,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut cache = CACHE_ITEM.lock().await;
        let mut self_ = self_.write().await;
        let Some(self__) = self_.take() else {
            bail!("Item is already being used in breaking operation.");
        };
        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("items");
        let filter =
            doc! {
            "GuildId": self__.guild_id.as_i64(),
            "ItemName": &self__.item_name,
        };
        if let Some(s) = session {
            collection.delete_one_with_session(filter, None, s).await?;
        } else {
            collection.delete_one(filter, None).await?;
        }
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

impl ToKVs for Item {}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_serialization() {
        let item = Item {
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
