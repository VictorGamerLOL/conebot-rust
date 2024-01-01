use crate::db::uniques::DbGuildId;

use super::{ DropTablePart, DropTablePartOption };

use anyhow::{ anyhow, Result };
use mongodb::{ bson::doc, ClientSession };

pub struct DropTablePartBuilder {
    guild_id: Option<DbGuildId>,
    drop_table_name: Option<String>,
    drop: Option<DropTablePartOption>,
    min: Option<i64>,
    max: Option<i64>,
    weight: Option<i64>,
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

    pub const fn guild_id(mut self, guild_id: DbGuildId) -> Self {
        self.guild_id = Some(guild_id);
        self
    }

    pub fn drop_table_name(mut self, drop_table_name: String) -> Self {
        self.drop_table_name = Some(drop_table_name);
        self
    }

    pub fn drop(mut self, drop: DropTablePartOption) -> Self {
        self.drop = Some(drop);
        self
    }

    pub const fn min(mut self, min: i64) -> Self {
        self.min = Some(min);
        self
    }

    pub const fn max(mut self, max: i64) -> Self {
        self.max = Some(max);
        self
    }

    pub const fn weight(mut self, weight: i64) -> Self {
        self.weight = Some(weight);
        self
    }

    pub(super) async fn build(self, session: Option<&mut ClientSession>) -> Result<DropTablePart> {
        let guild_id = self.guild_id.ok_or_else(|| anyhow!("Guild ID is missing"))?;
        let drop_table_name = self.drop_table_name.ok_or_else(||
            anyhow!("Drop table name is missing")
        )?;
        let db = crate::db::CLIENT.get().await.database("conebot");
        let collection = db.collection::<DropTablePart>("dropTables");

        let filter =
            doc! {
            "GuildId": guild_id.as_i64(),
            "DropTableName": &drop_table_name,
        };

        if collection.find_one(filter, None).await?.is_some() {
            return Err(anyhow!("Drop table part already exists"));
        }

        let part = DropTablePart {
            guild_id,
            drop_table_name,
            drop: self.drop.ok_or_else(|| anyhow!("Drop is missing"))?,
            min: self.min.unwrap_or(1),
            max: self.max,
            weight: self.weight.unwrap_or(1),
        };

        if let Some(session) = session {
            collection.insert_one(&part, None).await?;
        } else {
            collection.insert_one(&part, None).await?;
        }
        Ok(part)
    }
}
