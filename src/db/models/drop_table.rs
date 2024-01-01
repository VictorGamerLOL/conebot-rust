#![allow(clippy::module_name_repetitions)] // no.

use std::{ num::NonZeroUsize, ops::RangeInclusive };

use anyhow::Result;
use lazy_static::lazy_static;
use lru::LruCache;
use rand::distributions::uniform::SampleRange;
use serde::{ Deserialize, Serialize };
use tokio::sync::Mutex;

use crate::{
    db::{ uniques::DbGuildId, ArcTokioRwLockOption, TokioMutexCache },
    mechanics::drop_generator::{ DropGenerator, Droppable, DroppableKind },
};

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
    static ref DROP_TABLES_CACHE: TokioMutexCache<(String, String), ArcTokioRwLockOption<DropTable>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl From<DropTable> for DropGenerator<RangeInclusive<i64>> {
    fn from(drop_table: DropTable) -> Self {
        let mut drop_gen = Self::new();

        for part in drop_table.drop_table_parts {
            let (name, kind) = match part.drop {
                DropTablePartOption::Item { item_name } => (item_name, DroppableKind::Item),
                DropTablePartOption::Currency { currency_name } => {
                    (currency_name, DroppableKind::Currency)
                }
            };

            let range = match part.max {
                Some(max) => part.min..=max,
                None => part.min..=part.min,
            };

            let droppable = Droppable::new(name, kind, range, part.weight);

            drop_gen.add_droppable(droppable);
        }

        drop_gen
    }
}

impl DropTable {
    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn drop_table_name(&self) -> &String {
        &self.drop_table_name
    }

    pub const fn drop_table_parts(&self) -> &Vec<DropTablePart> {
        &self.drop_table_parts
    }
}

impl DropTablePart {
    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    pub const fn drop_table_name(&self) -> &String {
        &self.drop_table_name
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
