use crate::db::{ id::DbGuildId, ArcTokioRwLockOption };
use super::*;
use anyhow::{ Result, anyhow, bail };

pub struct Builder {
    guild_id: DbGuildId,
    item_name: String,
    description: Option<String>,
    sellable: Option<bool>,
    tradeable: Option<bool>,
    currency_value: Option<String>,
    value: Option<f64>,
    message: Option<String>,
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
            message: None,
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
            "guild_id": self.guild_id.to_string(),
            "item_name": self.item_name.clone()
        };
        let item = coll.find_one(filter, None).await?;
        if item.is_some() {
            bail!("Item already exists");
        }

        let description = self.description.unwrap_or_default();
        let sellable = self.sellable.unwrap_or(false);
        let tradeable = self.tradeable.unwrap_or(false);
        let currency_value = self.currency_value.unwrap_or_default();
        let value = self.value.unwrap_or(0.0);
        let message = self.message.unwrap_or_default();
        let item_type = self.item_type.unwrap_or(ItemType::Trophy);

        let item = Item {
            guild_id: self.guild_id.clone(),
            item_name: self.item_name.clone(),
            description,
            sellable,
            tradeable,
            currency_value,
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
}
