use std::sync::Arc;

use crate::db::{
    id::{DbChannelId, DbGuildId, DbRoleId},
    ArcTokioMutex,
};
use anyhow::{Ok, Result};
use mongodb::{bson::doc, Collection};

use super::Currency;

#[derive(Debug, Clone)]
pub struct CurrencyBuilder {
    guild_id: DbGuildId,
    curr_name: String,
    symbol: String,
    visible: Option<bool>,
    base: Option<bool>,
    base_value: Option<f64>,
    pay: Option<bool>,
    earn_by_chat: Option<bool>,
    channels_is_whitelist: Option<bool>,
    roles_is_whitelist: Option<bool>,
    channels_whitelist: Vec<DbChannelId>,
    roles_whitelist: Vec<DbRoleId>,
    channels_blacklist: Vec<DbChannelId>,
    roles_blacklist: Vec<DbRoleId>,
    earn_min: Option<f64>,
    earn_max: Option<f64>,
    earn_timeout: Option<i64>,
}

impl CurrencyBuilder {
    pub fn new(guild_id: DbGuildId, curr_name: String, symbol: String) -> Self {
        Self {
            guild_id,
            curr_name,
            symbol,
            visible: None,
            base: None,
            base_value: None,
            pay: None,
            earn_by_chat: None,
            channels_is_whitelist: None,
            roles_is_whitelist: None,
            channels_whitelist: Vec::new(),
            roles_whitelist: Vec::new(),
            channels_blacklist: Vec::new(),
            roles_blacklist: Vec::new(),
            earn_min: None,
            earn_max: None,
            earn_timeout: None,
        }
    }

    /// Builds the currency object and puts it into the database.
    /// It will retry up to 5 times before returning an error.
    ///
    /// # Examples
    /// ```rust
    /// use crate::db::models::currency::CurrencyBuilder;
    /// use crate::db::models::currency::Currency;
    /// use crate::db::id::{DbGuildId, DbChannelId, DbRoleId};
    ///
    /// let guild_id = DbGuildId::new(1234567890);
    /// let curr_name = String::from("Test Currency");
    /// let symbol = String::from("TC");
    ///
    /// let currency = CurrencyBuilder::new(guild_id, curr_name, symbol)
    ///    .build()
    ///    .await;
    ///
    /// assert_eq!(currency.guild_id, guild_id);
    /// assert_eq!(currency.curr_name, curr_name);
    /// assert_eq!(currency.symbol, symbol);
    /// assert_eq!(currency.visible, true);
    /// assert_eq!(currency.base, false);
    /// assert_eq!(currency.base_value, None);
    /// assert_eq!(currency.pay, true);
    /// assert_eq!(currency.earn_by_chat, false);
    /// assert_eq!(currency.channels_is_whitelist, false);
    /// assert_eq!(currency.roles_is_whitelist, false);
    /// assert_eq!(currency.channels_whitelist, Vec::<DbChannelId>::new());
    /// assert_eq!(currency.roles_whitelist, Vec::<DbRoleId>::new());
    /// assert_eq!(currency.channels_blacklist, Vec::<DbChannelId>::new());
    /// assert_eq!(currency.roles_blacklist, Vec::<DbRoleId>::new());
    /// assert_eq!(currency.earn_min, 1.0);
    /// assert_eq!(currency.earn_max, 10.0);
    /// assert_eq!(currency.earn_timeout, 30);
    /// ```
    pub async fn build(self) -> Result<ArcTokioMutex<Currency>> {
        // check if currency already exists
        let db = super::super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");
        let filter =
            doc! {"guild_id": self.guild_id.to_string(), "curr_name": self.curr_name.clone()};
        let curr = coll.find_one(filter, None).await?;
        if curr.is_some() {
            return Err(anyhow::anyhow!("Currency already exists"));
        }
        let guild_id = self.guild_id.clone();
        let curr_name = self.curr_name.clone();
        let symbol = self.symbol;
        let visible = self.visible.unwrap_or(true);
        let base = self.base.unwrap_or(false);
        let base_value = self.base_value;
        let pay = self.pay.unwrap_or(true);
        let earn_by_chat = self.earn_by_chat.unwrap_or(false);
        let channels_is_whitelist = self.channels_is_whitelist.unwrap_or(false);
        let roles_is_whitelist = self.roles_is_whitelist.unwrap_or(false);
        let channels_whitelist = self.channels_whitelist;
        let roles_whitelist = self.roles_whitelist;
        let channels_blacklist = self.channels_blacklist;
        let roles_blacklist = self.roles_blacklist;
        let earn_min = self.earn_min.unwrap_or(0.0);
        let earn_max = self.earn_max.unwrap_or(0.0);
        let earn_timeout = self.earn_timeout.unwrap_or(30);

        let curr = Currency {
            guild_id,
            curr_name,
            symbol,
            visible,
            base,
            base_value,
            pay,
            earn_by_chat,
            channels_is_whitelist,
            roles_is_whitelist,
            channels_whitelist,
            roles_whitelist,
            channels_blacklist,
            roles_blacklist,
            earn_min,
            earn_max,
            earn_timeout,
        };
        coll.insert_one(curr.clone(), None).await?;
        let arccurr: super::ArcTokioMutex<Currency> = Arc::new(tokio::sync::Mutex::new(curr));
        super::CACHE_CURRENCY
            .lock()
            .await
            .push((self.guild_id.to_string(), self.curr_name), arccurr.clone());
        Ok(arccurr)
    }

    /// Sets the guild_id field.
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn guild_id(mut self, guild_id: Option<DbGuildId>) -> Self {
        if let Some(guild_id) = guild_id {
            self.guild_id = guild_id;
        }
        self
    }
    /// Sets the curr_name field.
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn curr_name(mut self, curr_name: Option<String>) -> Self {
        if let Some(curr_name) = curr_name {
            self.curr_name = curr_name;
        }
        self
    }
    /// Sets the symbol field.
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn symbol(mut self, symbol: Option<String>) -> Self {
        if let Some(symbol) = symbol {
            self.symbol = symbol;
        }
        self
    }
    /// Sets the visible field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `true`
    /// upon calling the `build()` method.
    pub fn visible(mut self, visible: Option<bool>) -> Self {
        self.visible = visible;
        self
    }
    /// Sets the base field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn base(mut self, base: Option<bool>) -> Self {
        self.base = base;
        self
    }
    /// Sets the base_value field.
    /// If the method is not called, it falls back to `None`
    /// and shows up as `null` in the database.
    pub fn base_value(mut self, base_value: Option<f64>) -> Self {
        self.base_value = base_value;
        self
    }
    /// Sets the pay field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `true`
    pub fn pay(mut self, pay: Option<bool>) -> Self {
        self.pay = pay;
        self
    }
    /// Sets the earn_by_chat field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn earn_by_chat(mut self, earn_by_chat: Option<bool>) -> Self {
        self.earn_by_chat = earn_by_chat;
        self
    }
    /// Sets the channels_is_whitelist field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn channels_is_whitelist(mut self, channels_is_whitelist: Option<bool>) -> Self {
        self.channels_is_whitelist = channels_is_whitelist;
        self
    }
    /// Sets the roles_is_whitelist field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn roles_is_whitelist(mut self, roles_is_whitelist: Option<bool>) -> Self {
        self.roles_is_whitelist = roles_is_whitelist;
        self
    }
    /// Sets the channels_whitelist field.
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `channels_whitelist_add()`..
    pub fn channels_whitelist(mut self, channels_whitelist: Option<Vec<DbChannelId>>) -> Self {
        if let Some(channels_whitelist) = channels_whitelist {
            self.channels_whitelist = channels_whitelist;
        } else {
            self.channels_whitelist = vec![];
        };
        self
    }
    /// Adds a channel to the channels_whitelist field.
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `channels_whitelist()`.
    pub fn channels_whitelist_add(mut self, channel_id: DbChannelId) -> Self {
        self.channels_whitelist.push(channel_id);
        self
    }
    /// Sets the roles_whitelist field.
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `roles_whitelist_add()`.
    pub fn roles_whitelist(mut self, roles_whitelist: Option<Vec<DbRoleId>>) -> Self {
        if let Some(roles_whitelist) = roles_whitelist {
            self.roles_whitelist = roles_whitelist;
        } else {
            self.roles_whitelist = vec![];
        };
        self
    }
    /// Adds a role to the roles_whitelist field.
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `roles_whitelist()`.
    pub fn roles_whitelist_add(mut self, role_id: DbRoleId) -> Self {
        self.roles_whitelist.push(role_id);
        self
    }
    /// Sets the channels_blacklist field.
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `channels_blacklist_add()`.
    pub fn channels_blacklist(mut self, channels_blacklist: Option<Vec<DbChannelId>>) -> Self {
        if let Some(channels_blacklist) = channels_blacklist {
            self.channels_blacklist = channels_blacklist;
        } else {
            self.channels_blacklist = vec![];
        };
        self
    }
    /// Adds a channel to the channels_blacklist field.
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `channels_blacklist()`.
    pub fn channels_blacklist_add(mut self, channel_id: DbChannelId) -> Self {
        self.channels_blacklist.push(channel_id);
        self
    }
    /// Sets the roles_blacklist field.
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `roles_blacklist_add()`.
    pub fn roles_blacklist(mut self, roles_blacklist: Option<Vec<DbRoleId>>) -> Self {
        if let Some(roles_blacklist) = roles_blacklist {
            self.roles_blacklist = roles_blacklist;
        } else {
            self.roles_blacklist = vec![];
        };
        self
    }
    /// Adds a role to the roles_blacklist field.
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `roles_blacklist()`.
    pub fn roles_blacklist_add(mut self, role_id: DbRoleId) -> Self {
        self.roles_blacklist.push(role_id);
        self
    }
    /// Sets the earn_min field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `1.0`
    pub fn earn_min(mut self, earn_min: Option<f64>) -> Self {
        self.earn_min = earn_min;
        self
    }
    /// Sets the earn_max field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `10.0`
    pub fn earn_max(mut self, earn_max: Option<f64>) -> Self {
        self.earn_max = earn_max;
        self
    }
    /// Sets the earn_timeout field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `30`
    pub fn earn_timeout(mut self, earn_timeout: Option<i64>) -> Self {
        self.earn_timeout = earn_timeout;
        self
    }
}

#[tokio::test]
async fn test_currency_builder() {
    crate::init_env().await;
    let curr = CurrencyBuilder::new(DbGuildId::from(12), "ttest".to_owned(), "t".to_owned())
        .guild_id(Some(DbGuildId::from(123)))
        .curr_name(Some("test".to_string()))
        .symbol(Some("T".to_string()))
        .visible(Some(true))
        .base(Some(false))
        .base_value(Some(1.0))
        .pay(Some(true))
        .earn_by_chat(Some(true))
        .channels_is_whitelist(Some(true))
        .roles_is_whitelist(Some(true))
        .channels_whitelist(Some(vec![DbChannelId::from(123)]))
        .channels_whitelist_add(DbChannelId::from(456))
        .roles_whitelist(Some(vec![DbRoleId::from(123)]))
        .roles_whitelist_add(DbRoleId::from(456))
        .channels_blacklist(Some(vec![DbChannelId::from(123)]))
        .channels_blacklist_add(DbChannelId::from(456))
        .roles_blacklist(Some(vec![DbRoleId::from(123)]))
        .roles_blacklist_add(DbRoleId::from(456))
        .earn_min(Some(1.0))
        .earn_max(Some(10.0))
        .earn_timeout(Some(60))
        .build()
        .await
        .unwrap();
    let curr = curr.lock().await;
    let curr2 = curr.clone();
    drop(curr);
    let curr = curr2;

    assert_eq!(curr.guild_id, DbGuildId::from(123));
    assert_eq!(curr.curr_name, "test");
    assert_eq!(curr.symbol, "T");
    assert!(curr.visible);
    assert!(!curr.base);
    assert_eq!(curr.base_value, Some(1.0));
    assert!(curr.pay);
    assert!(curr.earn_by_chat);
    assert!(curr.channels_is_whitelist);
    assert!(curr.roles_is_whitelist);
    assert_eq!(
        curr.channels_whitelist,
        vec![DbChannelId::from(123), DbChannelId::from(456)]
    );
    assert_eq!(
        curr.roles_whitelist,
        vec![DbRoleId::from(123), DbRoleId::from(456)]
    );
    assert_eq!(
        curr.channels_blacklist,
        vec![DbChannelId::from(123), DbChannelId::from(456)]
    );
    assert_eq!(
        curr.roles_blacklist,
        vec![DbRoleId::from(123), DbRoleId::from(456)]
    );
    assert_eq!(curr.earn_min, 1.0);
    assert_eq!(curr.earn_max, 10.0);
    assert_eq!(curr.earn_timeout, 60);
}
