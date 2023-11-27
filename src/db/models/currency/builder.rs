use std::sync::Arc;

use crate::db::{ uniques::{ DbChannelId, DbGuildId, DbRoleId }, ArcTokioRwLockOption };
use anyhow::{ Ok, Result };
use chrono::Duration;
use mongodb::{ bson::doc, Collection };

use super::Currency;

#[derive(Debug, Clone)]
pub struct Builder {
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
    earn_timeout: Option<Duration>,
}

impl Builder {
    #[must_use]
    pub const fn new(guild_id: DbGuildId, curr_name: String, symbol: String) -> Self {
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
    ///
    /// # Errors
    ///
    /// Returns an error if the currency already exists, or if any mongodb operation errors.
    pub async fn build(self) -> Result<ArcTokioRwLockOption<Currency>> {
        // check if currency already exists
        let db = super::super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Currency> = db.collection("currencies");
        let filter = doc! { "GuildId": self.guild_id.as_i64(), "CurrName": &self.curr_name };
        let curr = coll.find_one(filter, None).await?;
        if curr.is_some() {
            return Err(anyhow::anyhow!("Currency already exists"));
        }
        let guild_id = self.guild_id;
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
        let earn_min = self.earn_min.unwrap_or(1.0);
        let earn_max = self.earn_max.unwrap_or(100.0);
        let earn_timeout = self.earn_timeout.unwrap_or_else(|| Duration::seconds(30));

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
        // if base is set to true, check if there is another currency where base is true and set it to false
        if curr.base {
            let filter = doc! { "GuildId": curr.guild_id.as_i64(), "Base": true };
            let update = doc! { "$set": {"Base": false} };
            coll.update_one(filter, update, None).await?;
        }

        let mut cache = super::CACHE_CURRENCY.lock().await;
        coll.insert_one(curr.clone(), None).await?;
        let arc_currency: ArcTokioRwLockOption<Currency> = Arc::new(
            tokio::sync::RwLock::new(Some(curr))
        );
        cache.push((self.guild_id, self.curr_name.clone()), arc_currency.clone());
        drop(cache);
        Ok(arc_currency)
    }

    /// `guild_id`
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn guild_id(&mut self, guild_id: impl Into<Option<DbGuildId>>) -> &mut Self {
        if let Some(guild_id) = guild_id.into() {
            self.guild_id = guild_id;
        }
        self
    }
    /// `curr_name`
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn curr_name(&mut self, curr_name: impl Into<Option<String>>) -> &mut Self {
        if let Some(curr_name) = curr_name.into() {
            self.curr_name = curr_name;
        }
        self
    }
    /// Sets the symbol field.
    /// If `None` is passed, the value provided with the `new()` method will be used.
    pub fn symbol(&mut self, symbol: impl Into<Option<String>>) -> &mut Self {
        if let Some(symbol) = symbol.into() {
            self.symbol = symbol;
        }
        self
    }
    /// Sets the visible field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `true`
    /// upon calling the `build()` method.
    pub fn visible(&mut self, visible: impl Into<Option<bool>>) -> &mut Self {
        self.visible = visible.into();
        self
    }
    /// Sets the base field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn base(&mut self, base: impl Into<Option<bool>>) -> &mut Self {
        self.base = base.into();
        self
    }
    /// `base_value`
    /// If the method is not called, it falls back to `None`
    /// and shows up as `null` in the database.
    pub fn base_value(&mut self, base_value: impl Into<Option<f64>>) -> &mut Self {
        self.base_value = base_value.into();
        self
    }
    /// Sets the pay field.
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `true`
    pub fn pay(&mut self, pay: impl Into<Option<bool>>) -> &mut Self {
        self.pay = pay.into();
        self
    }
    /// `earn_by_chat`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn earn_by_chat(&mut self, earn_by_chat: impl Into<Option<bool>>) -> &mut Self {
        self.earn_by_chat = earn_by_chat.into();
        self
    }
    /// `channels_is_whitelist`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn channels_is_whitelist(
        &mut self,
        channels_is_whitelist: impl Into<Option<bool>>
    ) -> &mut Self {
        self.channels_is_whitelist = channels_is_whitelist.into();
        self
    }
    /// `roles_is_whitelist`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `false`
    pub fn roles_is_whitelist(&mut self, roles_is_whitelist: impl Into<Option<bool>>) -> &mut Self {
        self.roles_is_whitelist = roles_is_whitelist.into();
        self
    }
    /// `channels_whitelist`
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `channels_whitelist_add()`..
    pub fn channels_whitelist(
        &mut self,
        channels_whitelist: impl Into<Option<Vec<DbChannelId>>>
    ) -> &mut Self {
        if let Some(channels_whitelist) = channels_whitelist.into() {
            self.channels_whitelist = channels_whitelist;
        } else {
            self.channels_whitelist = vec![];
        }
        self
    }
    /// `channels_whitelist`
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `channels_whitelist()`.
    pub fn channels_whitelist_add(&mut self, channel_id: DbChannelId) -> &mut Self {
        self.channels_whitelist.push(channel_id);
        self
    }
    /// `roles_whitelist`
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `roles_whitelist_add()`.
    pub fn roles_whitelist(
        &mut self,
        roles_whitelist: impl Into<Option<Vec<DbRoleId>>>
    ) -> &mut Self {
        if let Some(roles_whitelist) = roles_whitelist.into() {
            self.roles_whitelist = roles_whitelist;
        } else {
            self.roles_whitelist = vec![];
        }
        self
    }
    /// `roles_whitelist`
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `roles_whitelist()`.
    pub fn roles_whitelist_add(&mut self, role_id: DbRoleId) -> &mut Self {
        self.roles_whitelist.push(role_id);
        self
    }
    /// `channels_blacklist`
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `channels_blacklist_add()`.
    pub fn channels_blacklist(
        &mut self,
        channels_blacklist: impl Into<Option<Vec<DbChannelId>>>
    ) -> &mut Self {
        if let Some(channels_blacklist) = channels_blacklist.into() {
            self.channels_blacklist = channels_blacklist;
        } else {
            self.channels_blacklist = vec![];
        }
        self
    }
    /// `channels_blacklist`
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `channels_blacklist()`.
    pub fn channels_blacklist_add(&mut self, channel_id: DbChannelId) -> &mut Self {
        self.channels_blacklist.push(channel_id);
        self
    }
    /// `roles_blacklist`
    /// If `None` is passed, it resets the
    /// field to an empty vector.
    /// And if the method is not called,
    /// it falls back to anything provided
    /// with calls to `roles_blacklist_add()`.
    pub fn roles_blacklist(
        &mut self,
        roles_blacklist: impl Into<Option<Vec<DbRoleId>>>
    ) -> &mut Self {
        if let Some(roles_blacklist) = roles_blacklist.into() {
            self.roles_blacklist = roles_blacklist;
        } else {
            self.roles_blacklist = vec![];
        }
        self
    }
    /// `roles_blacklist`
    /// If the method is not called, it falls back to an empty vector,
    /// or the value provided with a previous call to `roles_blacklist()`.
    pub fn roles_blacklist_add(&mut self, role_id: DbRoleId) -> &mut Self {
        self.roles_blacklist.push(role_id);
        self
    }
    /// `earn_min`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `1.0`
    pub fn earn_min(&mut self, earn_min: impl Into<Option<f64>>) -> &mut Self {
        self.earn_min = earn_min.into();
        self
    }
    /// `earn_max`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `10.0`
    pub fn earn_max(&mut self, earn_max: impl Into<Option<f64>>) -> &mut Self {
        self.earn_max = earn_max.into();
        self
    }
    /// `earn_timeout`
    /// If `None` is passed, or the method is not called,
    /// it falls back to the default value of `30`
    pub fn earn_timeout(&mut self, earn_timeout: impl Into<Option<Duration>>) -> &mut Self {
        self.earn_timeout = earn_timeout.into();
        self
    }
}

#[tokio::test]
async fn test_currency_builder() {
    crate::init_env().await;
    let mut curr = Builder::new(DbGuildId::from(12u64), "TTest".to_owned(), "t".to_owned());
    curr.guild_id(Some(DbGuildId::from(123u64)))
        .curr_name(Some("test".to_owned()))
        .symbol(Some("T".to_owned()))
        .visible(Some(true))
        .base(Some(false))
        .base_value(Some(1.0))
        .pay(Some(true))
        .earn_by_chat(Some(true))
        .channels_is_whitelist(Some(true))
        .roles_is_whitelist(Some(true))
        .channels_whitelist(Some(vec![DbChannelId::from(123_i64)]))
        .channels_whitelist_add(DbChannelId::from(456_i64))
        .roles_whitelist(Some(vec![DbRoleId::from(123_i64)]))
        .roles_whitelist_add(DbRoleId::from(456_i64))
        .channels_blacklist(Some(vec![DbChannelId::from(123_i64)]))
        .channels_blacklist_add(DbChannelId::from(456_i64))
        .roles_blacklist(Some(vec![DbRoleId::from(123_i64)]))
        .roles_blacklist_add(DbRoleId::from(456_i64))
        .earn_min(Some(1.0))
        .earn_max(Some(10.0))
        .earn_timeout(Some(Duration::seconds(60)));
    let curr = curr.build().await.unwrap();
    let curr2 = curr.read().await;
    let curr3 = curr2.as_ref().unwrap();

    assert_eq!(curr3.guild_id, DbGuildId::from(123u64));
    assert_eq!(curr3.curr_name, "test");
    assert_eq!(curr3.symbol, "T");
    assert!(curr3.visible);
    assert!(!curr3.base);
    assert_eq!(curr3.base_value, Some(1.0));
    assert!(curr3.pay);
    assert!(curr3.earn_by_chat);
    assert!(curr3.channels_is_whitelist);
    assert!(curr3.roles_is_whitelist);
    assert_eq!(
        curr3.channels_whitelist,
        vec![DbChannelId::from(123_i64), DbChannelId::from(456_i64)]
    );
    assert_eq!(curr3.roles_whitelist, vec![DbRoleId::from(123_i64), DbRoleId::from(456_i64)]);
    assert_eq!(
        curr3.channels_blacklist,
        vec![DbChannelId::from(123_i64), DbChannelId::from(456_i64)]
    );
    assert_eq!(curr3.roles_blacklist, vec![DbRoleId::from(123_i64), DbRoleId::from(456_i64)]);

    let error_margin_f64 = f64::EPSILON;
    let res1 = curr3.earn_min - 1.0;
    let res2 = curr3.earn_max - 10.0;
    assert!(res1 < error_margin_f64);
    assert!(res2 < error_margin_f64);
    assert_eq!(curr3.earn_timeout, Duration::seconds(60));

    drop(curr2);

    Currency::delete_currency(curr).await.unwrap();
}
