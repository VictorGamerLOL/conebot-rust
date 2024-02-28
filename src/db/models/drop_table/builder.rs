use std::sync::Arc;

use crate::db::{ uniques::DbGuildId, ArcTokioRwLockOption };

use super::{ DropTable, DropTablePart, DropTablePartOption };

use anyhow::{ anyhow, Result };
use mongodb::{ bson::doc, ClientSession };
use tokio::sync::RwLock;

pub struct DropTablePartBuilder {
    pub(super) guild_id: Option<DbGuildId>,
    pub(super) drop_table_name: Option<String>,
    drop: Option<DropTablePartOption>,
    min: Option<i64>,
    max: Option<i64>,
    weight: Option<i64>,
}

pub struct DropTableBuilder {
    guild_id: Option<DbGuildId>,
    drop_table_name: Option<String>,
    drop_table_parts: Vec<DropTablePartBuilder>,
}

impl DropTablePartBuilder {
    pub const fn new() -> Self {
        Self {
            guild_id: None,
            drop_table_name: None,
            drop: None,
            min: None,
            max: None,
            weight: None,
        }
    }

    #[must_use]
    pub const fn guild_id(mut self, guild_id: Option<DbGuildId>) -> Self {
        self.guild_id = guild_id;
        self
    }

    pub fn byref_guild_id(&mut self, guild_id: Option<DbGuildId>) -> &mut Self {
        self.guild_id = guild_id;
        self
    }

    #[must_use]
    pub fn drop_table_name(mut self, drop_table_name: Option<String>) -> Self {
        self.drop_table_name = drop_table_name;
        self
    }

    pub fn byref_drop_table_name(&mut self, drop_table_name: Option<String>) -> &mut Self {
        self.drop_table_name = drop_table_name;
        self
    }

    #[must_use]
    pub fn drop(mut self, drop: Option<DropTablePartOption>) -> Self {
        self.drop = drop;
        self
    }

    pub fn byref_drop(&mut self, drop: Option<DropTablePartOption>) -> &mut Self {
        self.drop = drop;
        self
    }

    #[must_use]
    pub const fn min(mut self, min: Option<i64>) -> Self {
        self.min = min;
        self
    }

    pub fn byref_min(&mut self, min: Option<i64>) -> &mut Self {
        self.min = min;
        self
    }

    #[must_use]
    pub const fn max(mut self, max: Option<i64>) -> Self {
        self.max = max;
        self
    }

    pub fn byref_max(&mut self, max: Option<i64>) -> &mut Self {
        self.max = max;
        self
    }

    #[must_use]
    pub const fn weight(mut self, weight: Option<i64>) -> Self {
        self.weight = weight;
        self
    }

    pub fn byref_weight(&mut self, weight: Option<i64>) -> &mut Self {
        self.weight = weight;
        self
    }

    // pub super only because we do not want a stray DropTablePart to be created.
    pub(super) async fn build(self, session: Option<&mut ClientSession>) -> Result<DropTablePart> {
        let guild_id = self.guild_id.ok_or_else(|| anyhow!("Guild ID is missing"))?;
        let drop_table_name = self.drop_table_name.ok_or_else(||
            anyhow!("Drop table name is missing")
        )?;
        let drop = self.drop.ok_or_else(|| anyhow!("Drop is missing"))?;
        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let mut filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "DropTableName": &drop_table_name,
        };

        match drop {
            DropTablePartOption::Item { ref item_name } => {
                filter.insert("ItemName", item_name);
            }
            DropTablePartOption::Currency { ref currency_name } => {
                filter.insert("CurrencyName", currency_name);
            }
        }

        if collection.find_one(filter, None).await?.is_some() {
            return Err(anyhow!("Drop table part already exists"));
        }

        let part = DropTablePart {
            guild_id,
            drop_table_name,
            drop,
            min: self.min.unwrap_or(1),
            max: self.max,
            weight: self.weight.unwrap_or(1),
        };

        if let Some(s) = session {
            collection.insert_one_with_session(&part, None, s).await?;
        } else {
            collection.insert_one(&part, None).await?;
        }
        Ok(part)
    }
}

impl DropTableBuilder {
    pub const fn new() -> Self {
        Self {
            guild_id: None,
            drop_table_name: None,
            drop_table_parts: Vec::new(),
        }
    }

    #[must_use]
    /// Creates a new drop table part builder and adds it to the list of drop table parts.
    ///
    /// # Panics
    ///
    /// This function panics if the vector somehow failed pushing the new part.
    pub fn new_part(&mut self) -> &mut DropTablePartBuilder {
        let part = DropTablePartBuilder::new()
            .guild_id(self.guild_id)
            .drop_table_name(self.drop_table_name.clone());
        self.drop_table_parts.push(part);
        self.drop_table_parts.last_mut().unwrap()
    }

    #[must_use]
    pub fn guild_id(mut self, guild_id: Option<DbGuildId>) -> Self {
        self.guild_id = guild_id;
        self.drop_table_parts.iter_mut().for_each(|part| {
            part.byref_guild_id(guild_id);
        });
        self
    }

    #[must_use]
    pub fn drop_table_name(mut self, drop_table_name: Option<&str>) -> Self {
        self.drop_table_name = drop_table_name.map(ToOwned::to_owned);
        self.drop_table_parts.iter_mut().for_each(|part| {
            part.byref_drop_table_name(drop_table_name.map(ToOwned::to_owned));
        });
        self
    }

    /// Adds a drop table part to the list of drop table parts.
    ///
    /// # Panics
    ///
    /// This function panics if the vector fails to push the new part.
    ///
    /// # Errors
    ///
    /// This function returns an error if the guild ID or drop table name does not match.
    pub fn add_drop_table_part(
        mut self,
        mut drop_table_part: DropTablePartBuilder
    ) -> Result<Self> {
        if let Some(guild_id) = self.guild_id {
            if drop_table_part.guild_id.is_none() {
                drop_table_part.byref_guild_id(Some(guild_id));
            }
            if guild_id != drop_table_part.guild_id.unwrap() {
                return Err(anyhow!("Guild ID does not match"));
            }
        }
        if let Some(drop_table_name) = &self.drop_table_name {
            if drop_table_part.drop_table_name.is_none() {
                drop_table_part.byref_drop_table_name(Some(drop_table_name.clone()));
            }
            if drop_table_name != drop_table_part.drop_table_name.as_ref().unwrap() {
                return Err(anyhow!("Drop table name does not match"));
            }
        }
        self.drop_table_parts.push(drop_table_part);
        Ok(self)
    }

    #[must_use]
    pub fn clear_drop_table_parts(mut self) -> Self {
        self.drop_table_parts.clear();
        self
    }

    /// Builds the drop table.
    ///
    /// # Panics
    ///
    /// It will not the linter is stoopid.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is a problem building the drop table.
    pub async fn build(
        self,
        mut session: Option<&mut ClientSession>
    ) -> Result<ArcTokioRwLockOption<DropTable>> {
        let Some(guild_id) = self.guild_id else {
            return Err(anyhow!("Guild ID is missing"));
        };
        let Some(drop_table_name) = self.drop_table_name else {
            return Err(anyhow!("Drop table name is missing"));
        };

        let mut cache = super::DROP_TABLES_CACHE.lock().await;

        if let Some(drop_table) = cache.get(&(guild_id, drop_table_name.clone())) {
            let drop_table = drop_table.to_owned();
            let mut drop_table_ = drop_table.write().await;
            if let Some(drop_table__) = drop_table_.as_ref() {
                if !drop_table__.drop_table_parts().is_empty() {
                    return Err(anyhow!("Drop table already exists"));
                }
                drop_table_.take();
                drop(drop_table_);
                cache.pop(&(guild_id, drop_table_name.clone()));
            }
        }

        let mut owned_session: Option<ClientSession> = None;
        if session.is_none() {
            let client = crate::db::CLIENT.get().await;
            owned_session = Some(client.start_session(None).await?);
            session = owned_session.as_mut();
        }

        let mut parts: Vec<DropTablePart> = Vec::with_capacity(self.drop_table_parts.len());
        let session_ref = session.unwrap(); // It's guaranteed to be Some(thing) at this point
        session_ref.start_transaction(None).await?;
        for part_builder in self.drop_table_parts {
            let part = part_builder.build(Some(session_ref)).await?;
            parts.push(part);
        }
        if let Some(mut session) = owned_session {
            session.commit_transaction().await?;
        }
        let table = DropTable {
            guild_id,
            drop_table_name: drop_table_name.clone(),
            drop_table_parts: parts,
        };

        let arc: ArcTokioRwLockOption<DropTable> = Arc::new(RwLock::new(Some(table)));

        cache.put((guild_id, drop_table_name), arc.clone());

        drop(cache);
        Ok(arc)
    }
}
