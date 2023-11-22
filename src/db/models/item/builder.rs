use std::str::FromStr;

use super::*;
use crate::db::{ id::DbGuildId, ArcTokioRwLockOption };
use anyhow::{ anyhow, bail, Result };

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

        let mut db = crate::db::CLIENT.get().await.database("conebot");
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

        let mut db = crate::db::CLIENT.get().await.database("conebot");
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
    type_: Option<ItemTypeTypeBuilder>,
    message: Option<String>,
    action_type: Option<ActionTypeItemTypeBuilder>,
    role: Option<DbRoleId>,
    drop_table_name: Option<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
/// This exists because the builder needs to be able to
/// infer the type of the item, and the type of the item
/// needs to be able to be set by the builder. It can either
/// be set directly or inferred based on the presence of other
/// fields.
pub enum ItemTypeTypeBuilder {
    #[default]
    Trophy,
    Consumable,
    InstantConsumable,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
/// If the type of the item is not Trophy, then the action type
/// must be specified or be able to be inferred from the presence
/// of other fields. If neither `role` nor `drop_table_name` are
/// present, then the action type is `None`. If both are present,
/// then it is an error.
pub enum ActionTypeItemTypeBuilder {
    #[default]
    None,
    Role,
    Lootbox,
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

    pub fn build(mut self) -> Result<ItemType> {
        let action_type = self.infer_action_type()?;
        let type_ = self.type_.unwrap_or_else(|| {
            if
                self.message.is_some() ||
                self.action_type.is_some() ||
                self.role.is_some() ||
                self.drop_table_name.is_some()
            {
                ItemTypeTypeBuilder::Consumable
            } else {
                ItemTypeTypeBuilder::Trophy
            }
        });
        if type_ == ItemTypeTypeBuilder::Trophy {
            return Ok(Self::TROPHY);
        }

        let message = self.message.unwrap_or_default();
        let action_type: ItemActionType = match action_type {
            ActionTypeItemTypeBuilder::None => ItemActionType::None,
            ActionTypeItemTypeBuilder::Role => {
                let role = self.role.ok_or_else(||
                    anyhow!("Role must be present if action type is Role.")
                )?;
                ItemActionType::Role { role_id: role }
            }
            ActionTypeItemTypeBuilder::Lootbox => {
                let drop_table_name = self.drop_table_name.ok_or_else(|| {
                    anyhow!("Drop table name must be present if action type is Lootbox.")
                })?;
                ItemActionType::Lootbox { drop_table_name }
            }
        };

        match type_ {
            ItemTypeTypeBuilder::Trophy => Ok(ItemType::Trophy),
            ItemTypeTypeBuilder::Consumable =>
                Ok(ItemType::Consumable {
                    message,
                    action_type,
                }),
            ItemTypeTypeBuilder::InstantConsumable =>
                Ok(ItemType::InstantConsumable {
                    message,
                    action_type,
                }),
        }
    }

    pub fn infer_action_type(&self) -> Result<ActionTypeItemTypeBuilder> {
        if let Some(action_type) = self.action_type {
            return Ok(action_type);
        }
        if self.role.is_none() && self.drop_table_name.is_none() {
            Ok(ActionTypeItemTypeBuilder::None)
        } else if self.role.is_some() && self.drop_table_name.is_none() {
            Ok(ActionTypeItemTypeBuilder::Role)
        } else if self.role.is_none() && self.drop_table_name.is_some() {
            Ok(ActionTypeItemTypeBuilder::Lootbox)
        } else {
            Err(
                anyhow!(
                    "Role and drop_table_name cannot be both present when action type is not specified."
                )
            )
        }
    }

    pub fn type_(&mut self, type_: Option<ItemTypeTypeBuilder>) -> &mut Self {
        self.type_ = type_;
        self
    }

    pub fn message(&mut self, message: Option<String>) -> &mut Self {
        self.message = message;
        self
    }

    pub fn action_type(&mut self, action_type: Option<ActionTypeItemTypeBuilder>) -> &mut Self {
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

impl ToString for ItemTypeTypeBuilder {
    fn to_string(&self) -> String {
        match self {
            Self::Trophy => "Trophy".to_owned(),
            Self::Consumable => "Consumable".to_owned(),
            Self::InstantConsumable => "InstantConsumable".to_owned(),
        }
    }
}

impl FromStr for ItemTypeTypeBuilder {
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

impl ItemTypeTypeBuilder {
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

impl ToString for ActionTypeItemTypeBuilder {
    fn to_string(&self) -> String {
        match self {
            Self::None => "None".to_owned(),
            Self::Role => "Role".to_owned(),
            Self::Lootbox => "Lootbox".to_owned(),
        }
    }
}

impl FromStr for ActionTypeItemTypeBuilder {
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

impl ActionTypeItemTypeBuilder {
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

        let mut item_builder = Builder::new(guild_id, "existing_item".to_owned());
        assert!(item_builder.build().await.is_err());
    }
}
