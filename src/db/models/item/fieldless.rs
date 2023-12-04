use std::str::FromStr;

use crate::macros::const_impl;
use anyhow::{ anyhow, Result };
use serde::{ Deserialize, Serialize };

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
/// This exists because the builder needs to be able to
/// infer the type of the item, and the type of the item
/// needs to be able to be set by the builder. It can either
/// be set directly or inferred based on the presence of other
/// fields.
pub enum ItemTypeFieldless {
    #[default]
    Trophy,
    Consumable,
    InstantConsumable,
}

impl ToString for ItemTypeFieldless {
    fn to_string(&self) -> String {
        match self {
            Self::Trophy => "Trophy".to_owned(),
            Self::Consumable => "Consumable".to_owned(),
            Self::InstantConsumable => "InstantConsumable".to_owned(),
        }
    }
}
const_impl! {
    impl FromStr for ItemTypeFieldless {
        type Err = anyhow::Error;

        /// The from_str implementation on this is
        /// ***CASE SENSITIVE***. It assumes that the str passed
        /// is already lowercase. The only other case this works is
        /// if the first letter is capitalized.
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "trophy" | "Trophy" => Ok(Self::Trophy),
                "consumable" | "Consumable" => Ok(Self::Consumable),
                "instantconsumable" | "InstantConsumable" | "Instantconsumable" => {
                    Ok(Self::InstantConsumable)
                }
                _ => Err(anyhow!("Invalid item type: {}", s)),
            }
        }
    }
}

impl ItemTypeFieldless {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Trophy => "Trophy",
            Self::Consumable => "Consumable",
            Self::InstantConsumable => "InstantConsumable",
        }
    }

    pub fn from_string(mut s: String) -> Result<Self> {
        s.make_ascii_lowercase();
        match s.as_str() {
            "trophy" => Ok(Self::Trophy),
            "consumable" => Ok(Self::Consumable),
            "instantconsumable" | "instant consumable" | "instant-consumable" => {
                Ok(Self::InstantConsumable)
            }
            _ => Err(anyhow!("Invalid item type")),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
/// If the type of the item is not Trophy, then the action type
/// must be specified or be able to be inferred from the presence
/// of other fields. If neither `role` nor `drop_table_name` are
/// present, then the action type is `None`. If both are present,
/// then it is an error.
pub enum ItemActionTypeFieldless {
    #[default]
    None,
    Role,
    Lootbox,
}

impl ToString for ItemActionTypeFieldless {
    fn to_string(&self) -> String {
        match self {
            Self::None => "None".to_owned(),
            Self::Role => "Role".to_owned(),
            Self::Lootbox => "Lootbox".to_owned(),
        }
    }
}

const_impl! {
    impl FromStr for ItemActionTypeFieldless {
        type Err = anyhow::Error;

        /// The from_str implementation on this is
        /// ***CASE SENSITIVE***. It assumes that the str passed
        /// is already lowercase. The only other case this works is
        /// if the first letter is capitalized.
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "none" | "None" => Ok(Self::None),
                "role" | "Role" => Ok(Self::Role),
                "lootbox" | "Lootbox" => Ok(Self::Lootbox),
                _ => Err(anyhow!("Invalid action type: {}", s)),
            }
        }
    }
}

impl ItemActionTypeFieldless {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Role => "Role",
            Self::Lootbox => "Lootbox",
        }
    }

    pub fn from_string(mut s: String) -> Result<Self> {
        s.make_ascii_lowercase();
        match s.as_str() {
            "none" => Ok(Self::None),
            "role" => Ok(Self::Role),
            "lootbox" => Ok(Self::Lootbox),
            _ => Err(anyhow!("Invalid action type")),
        }
    }
}
