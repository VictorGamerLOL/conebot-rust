#![allow(clippy::module_name_repetitions)] // no.

use std::{ borrow::Cow, collections::HashMap, num::NonZeroUsize, ops::RangeInclusive, sync::Arc };

use anyhow::{ anyhow, Result };
use futures::{ future::{ self }, StreamExt, TryStreamExt };
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::{ bson::doc, ClientSession };
use serde::{ Deserialize, Serialize };
use tokio::sync::{ Mutex, RwLock, RwLockWriteGuard };

use crate::{
    db::{ uniques::{ DbGuildId, DropTableNameRef }, ArcTokioRwLockOption, TokioMutexCache, CLIENT },
    mechanics::drop_generator::{ DropGenerator, Droppable, DroppableKind },
};

use self::builder::DropTablePartBuilder;

pub mod builder;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct DropTable {
    guild_id: DbGuildId,
    drop_table_name: String,
    drop_table_parts: Vec<DropTablePart>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DropTablePart {
    guild_id: DbGuildId,
    drop_table_name: String,
    #[serde(flatten)]
    drop: DropTablePartOption,
    min: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<i64>,
    weight: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
#[serde(untagged)]
pub enum DropTablePartOption {
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))] Item {
        item_name: String,
    },
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))] Currency {
        currency_name: String,
    },
}

lazy_static! {
    static ref DROP_TABLES_CACHE: TokioMutexCache<(DbGuildId, String), ArcTokioRwLockOption<DropTable>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl From<&DropTable> for DropGenerator<RangeInclusive<i64>> {
    fn from(drop_table: &DropTable) -> Self {
        let mut drop_gen = Self::new();

        for part in drop_table.drop_table_parts.iter() {
            let (name, kind) = match part.drop.clone() {
                DropTablePartOption::Item { item_name } => (item_name, DroppableKind::Item),
                DropTablePartOption::Currency { currency_name } => {
                    (currency_name, DroppableKind::Currency)
                }
            };

            let range = part.max.map_or(part.min..=part.min, |max| part.min..=max);

            let droppable = Droppable::new(name, kind, range, part.weight);

            drop_gen.add_droppable(droppable);
        }

        drop_gen
    }
}

impl DropTable {
    /// Tries to get a drop table from the cache. Otherwise, tries to get it from the database.
    /// If nothing is returned from the database, the internal vector is empty. Check the length
    /// of drop_table_parts to see if it is empty.
    ///
    /// # Errors
    /// - Any mongodb error.
    pub async fn try_from_name(
        guild_id: DbGuildId,
        drop_table_name: Cow<'_, str>,
        session: Option<&mut ClientSession>
    ) -> Result<ArcTokioRwLockOption<Self>> {
        let mut cache = DROP_TABLES_CACHE.lock().await;

        if let Some(drop_table) = cache.get(&(guild_id, drop_table_name.clone().into_owned())) {
            return Ok(drop_table.to_owned());
        }

        let drop_table_parts = DropTablePart::try_from_name(
            guild_id,
            drop_table_name.clone(),
            session
        ).await?;

        if drop_table_parts.is_empty() {
            anyhow::bail!("Drop table not found.");
        }

        let drop_table = Self {
            guild_id,
            drop_table_name: drop_table_name.clone().into_owned(),
            drop_table_parts,
        };

        let arc: ArcTokioRwLockOption<Self> = Arc::new(RwLock::new(Some(drop_table)));

        cache.put((guild_id, drop_table_name.into_owned()), arc.clone());
        drop(cache);

        Ok(arc)
    }

    pub async fn bulk_update_part_item_name(
        guild_id: DbGuildId,
        before: &str,
        after: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let cache = DROP_TABLES_CACHE.lock().await;

        let rw_locks = cache
            .iter()
            .filter(|(k, _)| k.0 == guild_id)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>();

        // better to use a bit more memory than hold the ***entire cache*** hostage.

        drop(cache);

        // update drop tables
        for v in rw_locks {
            let mut v_lock = v.write().await;
            if let Some(v) = v_lock.as_mut() {
                for drop_table_part in &mut v.drop_table_parts {
                    match &mut drop_table_part.drop {
                        DropTablePartOption::Item { item_name } => {
                            if item_name == before {
                                *item_name = after.to_string();
                            }
                        }
                        DropTablePartOption::Currency { .. } => {}
                    }
                }
            }
            drop(v_lock);
        }
        // magically nothing above returns an error.

        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": before,
        };

        let update =
            doc! {
            "$set": {
                "ItemName": after,
            },
        };

        if let Some(s) = session {
            collection.update_many_with_session(filter, update, None, s).await?;
        } else {
            collection.update_many(filter, update, None).await?;
        }

        Ok(())
    }

    pub async fn bulk_update_part_currency_name(
        guild_id: DbGuildId,
        before: &str,
        after: String,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let cache = DROP_TABLES_CACHE.lock().await;

        let rw_locks = cache
            .iter()
            .filter(|(k, _)| k.0 == guild_id)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>();

        drop(cache);

        // update drop tables
        for v in rw_locks {
            let mut v_lock = v.write().await;
            if let Some(v) = v_lock.as_mut() {
                for drop_table_part in &mut v.drop_table_parts {
                    match &mut drop_table_part.drop {
                        DropTablePartOption::Item { .. } => {}
                        DropTablePartOption::Currency { currency_name } => {
                            if currency_name == before {
                                *currency_name = after.to_owned();
                            }
                        }
                    }
                }
            }
            drop(v_lock);
        }
        // magically nothing above returns an error.

        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "CurrencyName": before,
        };

        let update =
            doc! {
            "$set": {
                "CurrencyName": after,
            },
        };

        if let Some(s) = session {
            collection.update_many_with_session(filter, update, None, s).await?;
        } else {
            collection.update_many(filter, update, None).await?;
        }

        Ok(())
    }

    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn drop_table_name(&self) -> &String {
        &self.drop_table_name
    }

    pub const fn drop_table_parts(&self) -> &Vec<DropTablePart> {
        &self.drop_table_parts
    }

    /// Returns a new DropTablePartBuilder with the guild ID and drop table name set to this
    /// drop table's guild ID and drop table name.
    // mut self because why would you want to use this if you're not going to mutate it?
    #[must_use]
    pub fn new_part_builder(&mut self) -> DropTablePartBuilder {
        DropTablePartBuilder::new()
            .guild_id(Some(self.guild_id))
            .drop_table_name(Some(self.drop_table_name.clone()))
    }

    /// Adds a drop table part to the drop table.
    ///
    /// # Errors
    /// - If the guild ID or drop table name do not match the drop table's guild ID or drop table
    ///  name.
    /// - Any mongodb error.
    pub async fn add_part(
        &mut self,
        drop_table_part: DropTablePartBuilder,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if let Some(guild_id) = drop_table_part.guild_id {
            if guild_id != self.guild_id {
                return Err(anyhow!("Guild ID does not match"));
            }
        }
        if let Some(drop_table_name) = &drop_table_part.drop_table_name {
            if drop_table_name != &self.drop_table_name {
                return Err(anyhow!("Drop table name does not match"));
            }
        }

        let drop_table_part = drop_table_part.build(session).await?;

        self.drop_table_parts.push(drop_table_part);

        Ok(())
    }

    pub async fn delete(
        mut self_: RwLockWriteGuard<'_, Option<Self>>,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let mut cache = DROP_TABLES_CACHE.lock().await;

        let drop_table = self_
            .take()
            .ok_or_else(|| anyhow!("Drop table is being used in breaking operation."))?;

        drop(self_);

        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": drop_table.guild_id.as_i64(),
            "DropTableName": &drop_table.drop_table_name,
        };

        if let Some(s) = session {
            collection.delete_many_with_session(filter, None, s).await?;
        } else {
            collection.delete_many(filter, None).await?;
        }

        cache.pop(&(drop_table.guild_id, drop_table.drop_table_name));

        drop(cache);

        Ok(())
    }

    pub async fn delete_part(
        &mut self,
        drop_table_part_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let i: Option<usize> = self.drop_table_parts.iter().position(|drop_table_part| {
            match &drop_table_part.drop {
                DropTablePartOption::Item { item_name } => item_name == drop_table_part_name,
                DropTablePartOption::Currency { currency_name } => {
                    currency_name == drop_table_part_name
                }
            }
        });
        let Some(i) = i else {
            return Err(anyhow!("Drop table part not found."));
        };
        let part = self.drop_table_parts.swap_remove(i);
        part.delete(session).await?;
        Ok(())
    }

    /// Deletes all occurrences of a currency from all drop tables within a guild.
    pub async fn purge_currency(
        guild_id: DbGuildId,
        currency_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let cache = DROP_TABLES_CACHE.lock().await;

        let rw_locks = cache
            .iter()
            .filter(|(k, _)| k.0 == guild_id)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>();

        drop(cache);

        // remove currency from drop tables
        for v in rw_locks {
            let mut v_lock = v.write().await;
            if let Some(v) = v_lock.as_mut() {
                v.drop_table_parts.retain(|drop_table_part| {
                    match &drop_table_part.drop {
                        DropTablePartOption::Item { .. } => true,
                        DropTablePartOption::Currency { currency_name: c } => c != currency_name,
                    }
                });
            }
            drop(v_lock);
        }
        // magically nothing above returns an error.

        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "CurrencyName": currency_name,
        };

        if let Some(s) = session {
            collection.delete_many_with_session(filter, None, s).await?;
        } else {
            collection.delete_many(filter, None).await?;
        }

        Ok(())
    }

    /// Deletes all occurrences of an item from all drop tables within a guild.
    pub async fn purge_item(
        guild_id: DbGuildId,
        item_name: &str,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let cache = DROP_TABLES_CACHE.lock().await;

        // remove item from drop tables
        futures::stream
            ::iter(cache.iter())
            .filter(|(k, _)| future::ready(k.0 == guild_id))
            .map(|(_, v)| async {
                let mut v_lock = v.write().await;
                let Some(v) = v_lock.as_mut() else {
                    return;
                };
                v.drop_table_parts.retain(|drop_table_part| {
                    match &drop_table_part.drop {
                        DropTablePartOption::Item { item_name: i } => i != item_name,
                        DropTablePartOption::Currency { .. } => true,
                    }
                });
                drop(v_lock);
            })
            .buffer_unordered(10)
            .collect::<()>().await;
        // magically nothing above returns an error.

        drop(cache);

        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "ItemName": item_name,
        };

        if let Some(s) = session {
            collection.delete_many_with_session(filter, None, s).await?;
        } else {
            collection.delete_many(filter, None).await?;
        }

        Ok(())
    }

    /// Invalidates the cache of this drop table in a lenient way. This means that
    /// it will simply remove it from the cache without setting its its value to None
    /// beforehand. Useful when you want to dispose of an empty drop table.
    pub async fn invalidate_cache_lenient(&self) {
        let mut cache = DROP_TABLES_CACHE.lock().await;

        cache.pop(&(self.guild_id, self.drop_table_name.clone()));

        drop(cache);
    }

    /// Invalidates the cache of this drop table. This means that it will set its value
    /// to None before removing it from the cache. Useful when you want to dispose of
    /// a drop table properly.
    pub async fn invalidate_cache(mut self_: RwLockWriteGuard<'_, Option<Self>>) {
        let mut cache = DROP_TABLES_CACHE.lock().await;

        let drop_table = self_
            .take()
            .ok_or_else(|| anyhow!("Drop table is being used in breaking operation."));

        drop(self_);

        if let Ok(drop_table) = drop_table {
            cache.pop(&(drop_table.guild_id, drop_table.drop_table_name));
        }

        drop(cache);
    }
}

impl DropTablePart {
    pub async fn try_from_name(
        guild_id: DbGuildId,
        drop_table_name: Cow<'_, str>,
        session: Option<&mut ClientSession>
    ) -> Result<Vec<Self>> {
        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "DropTableName": drop_table_name.as_ref(),
        };

        if let Some(s) = session {
            Ok(collection.find_with_session(filter, None, s).await?.stream(s).try_collect().await?)
        } else {
            Ok(collection.find(filter, None).await?.try_collect().await?)
        }
    }

    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn drop_table_name(&self) -> DropTableNameRef<'_> {
        unsafe {
            DropTableNameRef::from_string_ref_and_guild_id_unchecked(
                self.guild_id,
                &self.drop_table_name
            )
        }
    }

    pub const fn drop(&self) -> &DropTablePartOption {
        &self.drop
    }

    pub const fn min(&self) -> i64 {
        self.min
    }

    pub const fn max(&self) -> Option<i64> {
        self.max
    }

    pub const fn weight(&self) -> i64 {
        self.weight
    }

    pub async fn delete(self, session: Option<&mut ClientSession>) -> Result<()> {
        let db = CLIENT.get().await.database("conebot");
        let collection = db.collection::<Self>("dropTables");

        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "DropTableName": self.drop_table_name,
        };

        if let Some(s) = session {
            collection.delete_one_with_session(filter, None, s).await?;
        } else {
            collection.delete_one(filter, None).await?;
        }

        Ok(())
    }
}

impl DropTablePartOption {
    pub const fn from_item_name(item_name: String) -> Self {
        Self::Item { item_name }
    }

    pub const fn from_currency_name(currency_name: String) -> Self {
        Self::Currency { currency_name }
    }

    pub const fn item_name(&self) -> Option<&String> {
        match self {
            Self::Item { item_name } => Some(item_name),
            Self::Currency { .. } => None,
        }
    }

    pub const fn currency_name(&self) -> Option<&String> {
        match self {
            Self::Item { .. } => None,
            Self::Currency { currency_name } => Some(currency_name),
        }
    }

    pub const fn name(&self) -> &String {
        match self {
            Self::Item { item_name } => item_name,
            Self::Currency { currency_name } => currency_name,
        }
    }

    pub const fn kind_as_str(&self) -> &'static str {
        match self {
            Self::Item { .. } => "item",
            Self::Currency { .. } => "currency",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_drop_table_part() {
        //item
        let drop_table_part = DropTablePart {
            guild_id: DbGuildId::from(1),
            drop_table_name: "test".to_string(),
            drop: DropTablePartOption::Item {
                item_name: "test".to_string(),
            },
            min: 1,
            max: Some(2),
            weight: 1,
        };

        let serialized = serde_json::to_string(&drop_table_part).unwrap();
        let json =
            r#"{"GuildId":1,"DropTableName":"test","ItemName":"test","Min":1,"Max":2,"Weight":1}"#;

        assert_eq!(serialized, json);

        //currency
        let drop_table_part = DropTablePart {
            guild_id: DbGuildId::from(1),
            drop_table_name: "test".to_string(),
            drop: DropTablePartOption::Currency {
                currency_name: "test".to_string(),
            },
            min: 1,
            max: Some(2),
            weight: 1,
        };

        let serialized = serde_json::to_string(&drop_table_part).unwrap();
        let json =
            r#"{"GuildId":1,"DropTableName":"test","CurrencyName":"test","Min":1,"Max":2,"Weight":1}"#;

        assert_eq!(serialized, json);

        //null max
        let drop_table_part = DropTablePart {
            guild_id: DbGuildId::from(1),
            drop_table_name: "test".to_string(),
            drop: DropTablePartOption::Currency {
                currency_name: "test".to_string(),
            },
            min: 1,
            max: None,
            weight: 1,
        };

        let serialized = serde_json::to_string(&drop_table_part).unwrap();
        let json =
            r#"{"GuildId":1,"DropTableName":"test","CurrencyName":"test","Min":1,"Max":null,"Weight":1}"#;

        assert_eq!(serialized, json);
    }
}
