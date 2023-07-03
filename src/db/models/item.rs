#![allow(clippy::module_name_repetitions)] // literally stop

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Item {
    guild_id: String,
    item_name: String,
    symbol: String,
    description: String,
    sellable: bool,
    tradeable: bool,
    currency_value: String,
    value: f64,
    #[serde(flatten)]
    item_type: ItemType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ItemType")]
pub enum ItemType {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ActionType")]
pub enum ItemActionType {
    None,
    #[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
    Role {
        role_id: String,
    },
    Lootbox {
        drop_table_name: String,
    },
}
