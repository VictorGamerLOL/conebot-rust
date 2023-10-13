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
            "GuildId": self.guild_id.to_string(),
            "ItemName": self.item_name.clone()
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

    pub fn message(&mut self, message: Option<String>) -> &mut Self {
        self.message = message;
        self
    }

    pub fn item_type(&mut self, item_type: Option<ItemType>) -> &mut Self {
        self.item_type = item_type;
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
        let item_name = "test_item".to_string();
        let item = Builder::new(guild_id.clone(), item_name.clone()).build().await.unwrap();
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
        let item_name = "test_item".to_string();
        let mut item_builder = Builder::new(guild_id.clone(), item_name.clone());
        item_builder.sellable(Some(true));
        assert!(item_builder.build().await.is_err());

        let mut item_builder = Builder::new(guild_id.clone(), "existing_item".to_owned());
        assert!(item_builder.build().await.is_err());
    }
}
