use std::sync::Arc;

use super::{
    fieldless::{ ItemActionTypeFieldless, ItemTypeFieldless },
    Item,
    ItemActionType,
    ItemType,
};
use crate::db::{ uniques::{ DbGuildId, DbRoleId }, ArcTokioRwLockOption };
use anyhow::{ anyhow, bail, Result };
use mongodb::bson::doc;
use serde::{ Deserialize, Serialize };

pub struct Builder {
    guild_id: DbGuildId,
    item_name: String,
    description: Option<String>,
    sellable: Option<bool>,
    tradeable: Option<bool>,
    currency_value: Option<String>,
    value: Option<f64>,
    item_type: Option<ItemType>,
}

impl Builder {
    pub const fn new(guild_id: DbGuildId, item_name: String) -> Self {
        Self {
            guild_id,
            item_name,
            description: None,
            sellable: None,
            tradeable: None,
            currency_value: None,
            value: None,
            item_type: None,
        }
    }

    pub async fn build(self) -> Result<ArcTokioRwLockOption<Item>> {
        if let Some(can_sell) = self.sellable {
            if (self.currency_value.is_none() || self.value.is_none()) && can_sell {
                bail!("Sellable items must have a currency value and a value");
            }
        }

        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll = db.collection::<Item>("items");

        let filter =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "ItemName": &self.item_name
        };
        let item = coll.find_one(filter, None).await?;
        if item.is_some() {
            bail!("Item already exists");
        }

        let description = self.description.unwrap_or_default();
        let sellable = self.sellable.unwrap_or(false);
        let tradeable = self.tradeable.unwrap_or(false);
        let currency_value = self.currency_value.unwrap_or_default();
        let value = self.value.unwrap_or_default();
        let item_type = self.item_type.unwrap_or(ItemType::Trophy);

        let item = Item {
            guild_id: self.guild_id,
            item_name: self.item_name.clone(),
            description,
            sellable,
            tradeable,
            currency: currency_value,
            value,
            item_type,
        };

        let db = crate::db::CLIENT.get().await.database("conebot");
        let coll = db.collection::<Item>("items");

        let mut cache = super::CACHE_ITEM.lock().await;
        coll.insert_one(item.clone(), None).await?;
        let item = Arc::new(tokio::sync::RwLock::new(Some(item)));
        cache.push((self.guild_id, self.item_name.clone()), item.clone());
        drop(cache);
        Ok(item)
    }

    pub fn description(&mut self, description: Option<String>) -> &mut Self {
        self.description = description;
        self
    }

    pub fn sellable(&mut self, sellable: Option<bool>) -> &mut Self {
        self.sellable = sellable;
        self
    }

    pub fn tradeable(&mut self, tradeable: Option<bool>) -> &mut Self {
        self.tradeable = tradeable;
        self
    }

    pub fn currency_value(&mut self, currency_value: Option<String>) -> &mut Self {
        self.currency_value = currency_value;
        self
    }

    pub fn value(&mut self, value: Option<f64>) -> &mut Self {
        self.value = value;
        self
    }

    pub fn item_type(&mut self, item_type: Option<ItemType>) -> &mut Self {
        self.item_type = item_type;
        self
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// There is a lot of inferring that can be done when
/// determining the item type in order to save the user
/// from having to specify a lot of fields. If we really
/// need more information, then we can just return an
/// error and have the user specify more fields.
pub struct ItemTypeBuilder {
    type_: Option<ItemTypeFieldless>,
    message: Option<String>,
    action_type: Option<ItemActionTypeFieldless>,
    role: Option<DbRoleId>,
    drop_table_name: Option<String>,
}

impl ItemTypeBuilder {
    /// To quickly make a trophy item rather than going through the builder.
    const TROPHY: ItemType = ItemType::Trophy;

    pub const fn new() -> Self {
        Self {
            type_: None,
            message: None,
            action_type: None,
            role: None,
            drop_table_name: None,
        }
    }

    pub fn build(self) -> Result<ItemType> {
        let action_type = self.infer_action_type()?;
        let type_ = self.type_.unwrap_or_else(|| {
            if
                self.message.is_some() ||
                self.action_type.is_some() ||
                self.role.is_some() ||
                self.drop_table_name.is_some()
            {
                ItemTypeFieldless::Consumable
            } else {
                ItemTypeFieldless::Trophy
            }
        });
        if type_ == ItemTypeFieldless::Trophy {
            return Ok(Self::TROPHY);
        }

        let message = self.message.unwrap_or_default();
        let action_type: ItemActionType = match action_type {
            ItemActionTypeFieldless::None => ItemActionType::None,
            ItemActionTypeFieldless::Role => {
                let role = self.role.ok_or_else(||
                    anyhow!("Role must be present if action type is Role.")
                )?;
                ItemActionType::Role { role_id: role }
            }
            ItemActionTypeFieldless::Lootbox => {
                let drop_table_name = self.drop_table_name.ok_or_else(|| {
                    anyhow!("Drop table name must be present if action type is Lootbox.")
                })?;
                ItemActionType::Lootbox { drop_table_name }
            }
        };

        match type_ {
            ItemTypeFieldless::Trophy => Ok(ItemType::Trophy),
            ItemTypeFieldless::Consumable =>
                Ok(ItemType::Consumable {
                    message,
                    action_type,
                }),
            ItemTypeFieldless::InstantConsumable =>
                Ok(ItemType::InstantConsumable {
                    message,
                    action_type,
                }),
        }
    }

    pub fn infer_action_type(&self) -> Result<ItemActionTypeFieldless> {
        if let Some(action_type) = self.action_type {
            return Ok(action_type);
        }
        if self.role.is_none() && self.drop_table_name.is_none() {
            Ok(ItemActionTypeFieldless::None)
        } else if self.role.is_some() && self.drop_table_name.is_none() {
            Ok(ItemActionTypeFieldless::Role)
        } else if self.role.is_none() && self.drop_table_name.is_some() {
            Ok(ItemActionTypeFieldless::Lootbox)
        } else {
            Err(
                anyhow!(
                    "Role and drop_table_name cannot be both present when action type is not specified."
                )
            )
        }
    }

    pub fn type_(&mut self, type_: Option<ItemTypeFieldless>) -> &mut Self {
        self.type_ = type_;
        self
    }

    pub fn message(&mut self, message: Option<String>) -> &mut Self {
        self.message = message;
        self
    }

    pub fn action_type(&mut self, action_type: Option<ItemActionTypeFieldless>) -> &mut Self {
        self.action_type = action_type;
        self
    }

    pub fn role(&mut self, role: Option<DbRoleId>) -> &mut Self {
        self.role = role;
        self
    }

    pub fn drop_table_name(&mut self, drop_table_name: Option<String>) -> &mut Self {
        self.drop_table_name = drop_table_name;
        self
    }
}
#[cfg(test)]
mod test {
    use std::io::Write;

    use crate::init_env;

    use super::*;

    #[tokio::test]
    async fn test_builder_and_delete() {
        init_env().await;
        let guild_id = DbGuildId::default();
        let item_name = "test_item".to_owned();
        let item = Builder::new(guild_id, item_name.clone()).build().await.unwrap();
        let item_ = item.read().await;
        assert!(item_.is_some());
        let item__ = item_.as_ref().unwrap();
        assert_eq!(item__.guild_id, guild_id);
        assert_eq!(item__.item_name, item_name);
        drop(item_);
        for i in 0..15 {
            print!("\rCheck the DB, {} second(s) remaining.  ", 15 - i);
            std::io::stdout().flush().unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        println!();
        super::super::Item
            ::delete_item(item).await
            .expect("Failed to delete item.\n !!THE ITEM MUST NOW BE DELETED MANUALLY!!");
    }

    #[tokio::test]
    async fn test_builder_safety() {
        init_env().await;
        let guild_id = DbGuildId::default();
        let item_name = "test_item".to_owned();
        let mut item_builder = Builder::new(guild_id, item_name.clone());
        item_builder.sellable(Some(true));
        assert!(item_builder.build().await.is_err());

        let item_builder = Builder::new(guild_id, "existing_item".to_owned());
        assert!(item_builder.build().await.is_err());
    }
}
