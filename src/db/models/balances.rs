//! This module contains the Balance and Balances structs and their methods.
//!
//! This is the main way that the bot stores the balances of each currency for each guild member.
//! Balances are relatively simple as they just consist of a guild id, a user id, the currency name,
//! and the amount of that currency that the user has.
//!
//! The only operations that one might want to do with balances is modify how much of one currency
//! a user has, initialize a new balance, get how much of one currency the user has or delete that
//! balance altogether. So therefore there is a relatively low amount of methods as you should not
//! be doing things like changing the user that has that amount of currency or changing the currency.
//!

use crate::db::uniques::{ CurrencyNameRef, DbGuildId, DbUserId };
use crate::db::{ ArcTokioMutexOption, ArcTokioRwLockOption, TokioMutexCache };
use anyhow::{ anyhow, Result };
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::bson::doc;
use mongodb::{ ClientSession, Collection };
use serde::{ Deserialize, Serialize };
use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::{ Mutex, MutexGuard };

use super::Currency;

/// This struct represents all of the balances for every currency for a certain user in a certain
/// guild.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Balances {
    guild_id: DbGuildId,
    user_id: DbUserId,
    balances: Vec<Balance>,
}

/// This struct represents the balance for a certain user in a specific guild for a specific currency.
///
/// This struct **assumes** that it is present within a `Balances` struct, and assumes that it will only
/// be created by a balances struct, but to prevent stupid things, clone has not been implemented
/// for it and the Balances struct if it wishes to make another it should use `transmute_copy`.
#[allow(clippy::unsafe_derive_deserialize)] // Shush I know what I'm doing.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Balance {
    guild_id: DbGuildId,
    user_id: DbUserId,
    pub curr_name: String,
    pub amount: f64,
}

lazy_static! {
    pub static ref CACHE_BALANCES: TokioMutexCache<(DbGuildId, DbUserId), ArcTokioMutexOption<Balances>> =
        Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()));
}

impl Balances {
    /// Attempts to fetch a user's balances from the cache or the database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    pub async fn try_from_user(
        guild_id: DbGuildId,
        user_id: DbUserId
    ) -> Result<ArcTokioMutexOption<Self>> {
        let mut cache = CACHE_BALANCES.lock().await;
        let balances = cache.get(&(guild_id, user_id)).cloned();
        // if it some return if, else continue
        if let Some(balances) = balances {
            return Ok(balances);
        }

        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
        };
        let res = coll.find(filterdoc, None).await?;
        let res = TryStreamExt::try_collect::<Vec<Balance>>(res).await?;
        drop(db);

        let balances = Arc::new(
            Mutex::new(
                Some(Self {
                    guild_id,
                    user_id,
                    balances: res,
                })
            )
        );
        cache.put((guild_id, user_id), balances.clone());
        drop(cache);
        Ok(balances)
    }

    pub async fn delete_currency(currency: &Currency) -> Result<()> {
        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
        let filterdoc =
            doc! {
            "GuildId": currency.guild_id().as_i64(),
            "CurrName": &currency.curr_name().as_str(),
        };
        coll.delete_many(filterdoc, None).await?;
        Ok(())
    }

    #[allow(clippy::must_use_candidate)]
    pub fn balances(&self) -> &[Balance] {
        &self.balances
    }

    #[allow(clippy::must_use_candidate)]
    pub fn balances_mut(&mut self) -> &mut [Balance] {
        &mut self.balances
    }

    /// Adds another balance for this user for a certain currency.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    /// - The user already has a balance for that currency in that guild.
    pub async fn create_balance(&mut self, curr_name: String) -> Result<&mut Balance> {
        let bal = Balance::new(self.guild_id, self.user_id, curr_name.clone()).await?;
        self.balances.push(bal);

        self.balances
            .iter_mut()
            .find(|b| b.curr_name() == curr_name)
            .ok_or_else(|| anyhow!("Created balance but could not find it afterwards, strange."))
    }

    #[allow(clippy::must_use_candidate)]
    pub fn has_currency(&self, curr_name: &str) -> bool {
        self.balances.iter().any(|bal| bal.curr_name == curr_name)
    }

    /// Checks if the user has a balance of a certain currency, and if they don't,
    /// make a balance for said currency. Then returns it.
    ///
    /// # Errors
    /// - Any `MongoDB` error occurs.
    pub async fn ensure_has_currency(&mut self, curr_name: Cow<'_, str>) -> Result<&mut Balance> {
        if let Some(i) = self.balances.iter().position(|b| b.curr_name == curr_name) {
            return Ok(&mut self.balances[i]);
        }
        self.create_balance(curr_name.into_owned()).await
    }

    /// Delete the balance for the specified currency for the user in the guild.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    /// - If the amount of deleted documents is 0.
    pub async fn delete_balance(&mut self, curr_name: &str) -> Result<()> {
        let db = super::super::CLIENT.get().await.database("conebot");
        let _coll: Collection<Balance> = db.collection("balances");
        // get the balance with the specified name from self's balance vec as owned value
        let bal = self.balances
            .iter()
            .position(|bal| bal.curr_name == curr_name)
            .ok_or_else(|| anyhow!("No balance with that currency name exists"))?;
        let bal = self.balances.remove(bal);
        // delete the balance from the database
        let res = bal.delete().await;
        if let Err((e, bal)) = res {
            // if there was an error, put the balance back into the vec
            self.balances.push(bal);
            return Err(e);
        }
        Ok(())
    }

    pub async fn bulk_update_currency_name(
        guild_id: DbGuildId,
        before: &str,
        after: String,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        let cache = CACHE_BALANCES.lock().await;

        let cache_iter = cache
            .iter()
            .filter_map(|(k, v)| {
                if k.0 == guild_id { Some(v.to_owned()) } else { None }
            })
            .collect::<Vec<_>>();

        drop(cache);

        for balances in cache_iter {
            let mut lock_res = balances.lock().await;
            if let Some(balances) = lock_res.as_mut() {
                for balance in &mut balances.balances {
                    if balance.curr_name == before {
                        balance.curr_name = after.clone();
                    }
                }
            }
            drop(lock_res);
        }

        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "CurrName": before,
        };
        let updatedoc =
            doc! {
            "$set": {
                "CurrName": after,
            },
        };
        if let Some(session) = session {
            coll.update_many_with_session(filterdoc, updatedoc, None, session).await?;
        } else {
            coll.update_many(filterdoc, updatedoc, None).await?;
        }
        Ok(())
    }

    /// Gets all of the balances from a guild, then looks
    /// inside of them to see if they have a balance for the specified currency.
    /// If they do, it deletes it. Effectively deleting the currency from the guild.
    ///
    /// Basically a makeshift cascading delete for currencies.
    ///
    /// # Errors
    /// - Any `MongoDB` error occurs.
    pub async fn purge_currency(guild_id: DbGuildId, curr_name: CurrencyNameRef<'_>) -> Result<()> {
        let mut cache = CACHE_BALANCES.lock().await;
        let cache_iter = cache.iter_mut();

        for (k, v) in cache_iter {
            if k.0 != guild_id {
                continue;
            }
            let mut lock_res = v.lock().await;
            if let Some(balances) = lock_res.as_mut() {
                balances.balances.retain(|bal| bal.curr_name != curr_name);
            }
            drop(lock_res);
        }
        drop(cache);

        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "CurrName": curr_name.as_str(),
        };
        coll.delete_many(filterdoc, None).await?;
        Ok(())
    }

    pub async fn invalidate_cache(mut self_: MutexGuard<'_, Option<Self>>) -> Result<()> {
        let take_res = self_.take();
        let Some(self__) = take_res else {
            return Err(anyhow!("Balances are being used in a breaking operation."));
        };
        let mut cache = CACHE_BALANCES.lock().await;
        cache.pop(&(self__.guild_id, self__.user_id));
        drop(cache);
        drop(self_);
        Ok(())
    }
}

impl Balance {
    /// Attempts to make a new balance corresponding to a specific user, currency and guild.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    /// - The user already has a balance for that currency in that guild.
    pub async fn new(guild_id: DbGuildId, user_id: DbUserId, curr_name: String) -> Result<Self> {
        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": guild_id.as_i64(),
            "UserId": user_id.as_i64(),
            "CurrName": &curr_name,
        };
        let res = coll.find_one(filterdoc, None).await?;
        if res.is_some() {
            return Err(anyhow!("User already has a balance for that currency in that guild."));
        }

        let user_balance = Self {
            guild_id,
            user_id,
            curr_name,
            amount: 0.0,
        };

        coll.insert_one(&user_balance, None).await?;
        Ok(user_balance)
    }

    /// Attempts to get the currency corresponding to this balance.
    /// # Errors
    /// - Any `MongoDB` error occurs.
    /// - The currency does not exist.
    pub async fn currency(&self) -> Result<Option<ArcTokioRwLockOption<Currency>>> {
        Currency::try_from_name(self.guild_id, self.curr_name.clone()).await
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn guild_id(&self) -> DbGuildId {
        self.guild_id
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn user_id(&self) -> DbUserId {
        self.user_id
    }

    #[allow(clippy::must_use_candidate)]
    pub fn curr_name(&self) -> &str {
        &self.curr_name
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn amount(&self) -> f64 {
        self.amount
    }
    /// Sets the amount of the currency that the user said to the specified amount.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - If the amount specified is infinite.
    /// - If the amount specified is NaN.
    #[inline]
    pub async fn set_amount(
        &mut self,
        amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_infinite() {
            return Err(anyhow!("Amount cannot be infinite."));
        }
        if amount.is_nan() {
            return Err(anyhow!("Amount cannot be NaN."));
        }

        self.set_amount_unchecked(amount, session).await
    }

    /// Adds the specified amount to the current amount.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount is negative.
    /// - The specified amount is infinite.
    /// - The specified amount would cause the balance to overflow to infinity.
    /// - The specified amount would cause a NaN.
    #[allow(clippy::float_cmp)] // we compare ***TRUNCATED*** values.
    pub async fn add_amount(
        &mut self,
        mut amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot add NaN."));
        }
        if amount < 0.0 {
            return Err(anyhow!("Cannot add a negative amount."));
        }
        if amount.is_infinite() {
            return Err(anyhow!("Cannot add infinity."));
        }
        // Round provided amount to 2 decimal places
        amount = if amount.trunc() == amount || (amount * 100.0).trunc() == amount * 100.0 {
            amount
        } else {
            (amount * 100.0).round() * 0.01 // multiplication is faster than division
        };
        let new_amount = self.amount.mul_add(100.0, amount * 100.0).round() * 0.01;
        if new_amount.is_infinite() {
            return Err(anyhow!("Cannot add that amount, would overflow to infinity."));
        } else if new_amount.is_nan() {
            return Err(anyhow!("Cannot add that amount, would cause a NaN."));
        }
        self.set_amount(new_amount, session).await
    }

    /// Subtracts the specified amount from the current amount.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - If the amount to subtract is greater than the current amount.
    /// - The specified amount is negative.
    /// - The specified amount is infinite.
    /// - The specified amount would cause a NaN.
    pub async fn sub_amount(
        &mut self,
        mut amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot subtract NaN."));
        }
        if amount < 0.0 {
            return Err(anyhow!("Cannot subtract a negative amount."));
        }
        if amount.is_infinite() {
            return Err(anyhow!("Cannot subtract infinity."));
        }
        amount = (amount * 100.0).round() * 0.01; // multiplication is faster than division
        if amount > self.amount {
            return Err(anyhow!("Cannot subtract more than the current amount."));
        }
        let new_amount = self.amount.mul_add(100.0, -amount * 100.0).round() * 0.01;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot subtract that amount, would cause a NaN."));
        }
        self.set_amount(new_amount, session).await
    }

    /// Subtracts the specified amount from the current amount without checking if the balance
    /// will go into the negatives and without checking if the amount is negative. However, it still
    /// checks to see if the result of the operations turns out to be NaN.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount would cause a NaN.
    pub async fn sub_amount_unchecked(
        &mut self,
        mut amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot subtract NaN."));
        }
        amount = (amount * 100.0).round() / 100.0;
        let new_amount = self.amount.mul_add(100.0, -amount * 100.0).round() / 100.0;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot subtract that amount, would cause a NaN."));
        }
        self.set_amount_unchecked(new_amount, session).await
    }

    /// Adds the specified amount to the current amount without checking if the balance
    /// will go into infinity and without checking if the amount is negative. However, it still
    /// checks to see if the result of the operations turns out to be NaN.
    ///
    /// # Errors
    ///
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount would cause a NaN.
    pub async fn add_amount_unchecked(
        &mut self,
        mut amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot add NaN."));
        }
        amount = (amount * 100.0).round() / 100.0;
        let new_amount = self.amount.mul_add(100.0, amount * 100.0).round() / 100.0;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot add that amount, would cause a NaN."));
        }
        self.set_amount_unchecked(new_amount, session).await
    }

    /// Sets the amount to the specified amount without checking if the amount is infinite.
    /// However, it still checks to see if the amount is NaN.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount is NaN.
    pub async fn set_amount_unchecked(
        &mut self,
        mut amount: f64,
        session: Option<&mut ClientSession>
    ) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot set NaN."));
        }
        // -0 on a balance sheet looks a bit odd, so we just set it to 0
        // praying that somehow it is not a special kind of -0 that cannot
        // be compared to this -0 we have here.
        if amount == -0.0 {
            amount = 0.0;
        }
        amount = (amount * 100.0).round() / 100.0;
        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "UserId": self.user_id.as_i64(),
            "CurrName": self.curr_name.as_str(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "Amount": amount,
            },
        };

        let res;
        if let Some(session) = session {
            res = coll.update_one_with_session(filterdoc, updatedoc, None, session).await?;
        } else {
            res = coll.update_one(filterdoc, updatedoc, None).await?;
        }
        if res.modified_count == 0 {
            return Err(anyhow!("Failed to update balance."));
        }
        self.amount = amount;
        Ok(())
    }

    /// Clears the user's balance for this currency.
    ///
    /// Literally just an alias for `set_amount(0.0)`.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    #[inline]
    pub async fn clear(&mut self, session: Option<&mut ClientSession>) -> Result<()> {
        self.set_amount(0.0, session).await
    }

    /// Checks if this balance belongs to a valid currency.
    ///
    /// It does this by trying to get the currency by its name.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    pub async fn is_valid(&self) -> Result<bool> {
        Ok(
            super::currency::Currency
                ::try_from_name(self.guild_id, self.curr_name.clone()).await?
                .is_some()
        )
    }

    /// Deletes this user's balance for the specific
    /// currency in the specific guild from the database.
    ///
    /// When it fails, it returns the balance back.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of deleted documents is 0.
    pub async fn delete(self) -> Result<(), (anyhow::Error, Self)> {
        let db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Self> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.as_i64(),
            "UserId": self.user_id.as_i64(),
            "CurrName": &self.curr_name,
        };

        let res = match coll.delete_one(filterdoc, None).await {
            Ok(res) => res,
            Err(e) => {
                return Err((e.into(), self));
            }
        };
        if res.deleted_count == 0 {
            return Err((anyhow!("No documents were deleted"), self));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    const TEST_USER_ID: u64 = 987_654_321;
    const TEST_GUILD_ID: u64 = 123_456_789;
    #[tokio::test]
    async fn test_try_from_user() {
        crate::init_env().await;
        let user = crate::db::uniques::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::uniques::DbGuildId::from(TEST_GUILD_ID);
        let balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let balances_ = balances.as_mut().unwrap();
        assert_eq!(balances_.balances.len(), 2); // There are 2 test currencies in the DB matching the IDs
        drop(balances); // please the clippy nursery
    }

    #[tokio::test]
    #[allow(clippy::float_cmp)]
    async fn test_checked_amount_operations() {
        crate::init_env().await;
        let user = crate::db::uniques::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::uniques::DbGuildId::from(TEST_GUILD_ID);
        let balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let balances_ = balances.as_mut().unwrap();
        let balance = balances_.balances
            .iter_mut()
            .find(|b| b.curr_name == "test")
            .unwrap();
        let error_margin: f64 = f64::EPSILON;

        balance.set_amount(30.0, None).await.ok();

        assert!((balance.amount - 30.0).abs() < error_margin); // value in DB is 30.0
        balance.add_amount(1.0, None).await.unwrap();
        assert!((balance.amount - 31.0).abs() < error_margin);
        balance.sub_amount(1.0, None).await.unwrap();
        assert!((balance.amount - 30.0).abs() < error_margin);

        assert!(balance.add_amount(f64::INFINITY, None).await.is_err()); // inf check
        assert!(balance.sub_amount(f64::INFINITY, None).await.is_err());

        assert!(balance.add_amount(f64::MAX, None).await.is_err()); // overflow check
        assert!(balance.sub_amount(32.0, None).await.is_err());

        assert!(balance.add_amount(-1.0, None).await.is_err()); // negative check
        assert!(balance.sub_amount(-1.0, None).await.is_err());

        assert!(balance.add_amount(f64::NAN, None).await.is_err()); // NaN check
        assert!(balance.sub_amount(f64::NAN, None).await.is_err());

        balance.set_amount(0.1, None).await.unwrap(); // Rounding to 2dp check.
        balance.add_amount(0.2, None).await.unwrap(); // Precision cmp is required here without error margin.
        assert_eq!(balance.amount, 0.3);
        balance.sub_amount(0.2, None).await.unwrap();
        assert_eq!(balance.amount, 0.1);

        balance.set_amount(0.111, None).await.unwrap(); // Rounding to 2dp check part 2.
        assert_eq!(balance.amount, 0.11);
        balance.add_amount(0.222, None).await.unwrap();
        assert_eq!(balance.amount, 0.33);
        balance.sub_amount(0.222, None).await.unwrap();
        assert_eq!(balance.amount, 0.11);

        balance.set_amount(30.0, None).await.ok(); // Reset amount

        drop(balances);
    }

    #[tokio::test]
    async fn test_unchecked_amount_operations() {
        crate::init_env().await;
        let user = crate::db::uniques::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::uniques::DbGuildId::from(TEST_GUILD_ID);
        let balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let balances_ = balances.as_mut().unwrap();
        let balance = balances_.balances
            .iter_mut()
            .find(|b| b.curr_name == "test")
            .unwrap();
        let error_margin: f64 = f64::EPSILON;
        assert!(balance.amount - 30.0 < error_margin);

        balance.add_amount_unchecked(f64::INFINITY, None).await.unwrap();
        assert!(balance.amount.is_infinite() && balance.amount.is_sign_positive());
        balance.set_amount(30.0, None).await.unwrap();
        balance.sub_amount_unchecked(f64::INFINITY, None).await.unwrap();
        assert!(balance.amount.is_infinite() && balance.amount.is_sign_negative());
        balance.set_amount(30.0, None).await.unwrap();

        balance.add_amount_unchecked(-1.0, None).await.unwrap();
        assert!(balance.amount - 29.0 < error_margin);
        balance.sub_amount_unchecked(-1.0, None).await.unwrap();
        assert!(balance.amount - 30.0 < error_margin);

        assert!(balance.add_amount_unchecked(f64::NAN, None).await.is_err()); // NaN check
        assert!(balance.sub_amount_unchecked(f64::NAN, None).await.is_err());

        drop(balances);
    }
}
