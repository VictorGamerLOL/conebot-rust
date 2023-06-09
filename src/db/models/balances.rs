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

use crate::db::id::{ DbGuildId, DbUserId };
use crate::db::{ ArcTokioMutexOption, TokioMutexCache };
use anyhow::{ anyhow, Result };
use futures::TryStreamExt;
use lazy_static::lazy_static;
use lru::LruCache;
use mongodb::bson::doc;
use mongodb::Collection;
use serde::{ Deserialize, Serialize };
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::Currency;

/// This struct represents all of the balances for every currency for a certain user in a certain
/// guild.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct Balances {
    guild_id: DbGuildId,
    user_id: DbUserId,
    pub balances: Vec<Balance>,
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
        let balances = cache.get(&(guild_id.clone(), user_id.clone())).cloned();
        // if it some return if, else continue
        if let Some(balances) = balances {
            return Ok(balances);
        }

        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
        let filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
        };
        let res = coll.find(filterdoc, None).await?;
        let res = TryStreamExt::try_collect::<Vec<Balance>>(res).await?;
        drop(db);

        let balances = Arc::new(
            Mutex::new(
                Some(Balances {
                    guild_id: guild_id.clone(),
                    user_id: user_id.clone(),
                    balances: res,
                })
            )
        );
        cache.put((guild_id, user_id), balances.clone());
        Ok(balances)
    }

    pub fn balances(&self) -> &[Balance] {
        return &self.balances;
    }

    /// Adds another balance for this user for a certain currency.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    /// - The user already has a balance for that currency in that guild.
    pub async fn create_balance(&mut self, curr_name: String) -> Result<()> {
        let bal = Balance::new(self.guild_id.clone(), self.user_id.clone(), curr_name).await?;
        self.balances.push(bal);
        Ok(())
    }

    #[allow(clippy::must_use_candidate)]
    pub fn has_currency(&self, curr_name: &str) -> bool {
        self.balances.iter().any(|bal| bal.curr_name == curr_name)
    }

    /// Delete the balance for the specified currency for the user in the guild.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any `MongoDB` error occurs.
    /// - If the amount of deleted documents is 0.
    pub async fn delete_balance(&mut self, curr_name: &str) -> Result<()> {
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");
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
        let user_balance = Self {
            guild_id: guild_id.clone(),
            user_id: user_id.clone(),
            curr_name: curr_name.clone(),
            amount: 0.0,
        };
        // mem copy into another since there is no clone
        let user_balance2: Balance = unsafe { std::mem::transmute_copy(&user_balance) };
        /*
         * Since there is no clone, unsafe comes to the rescue! Because I cannot allow myself to
         * clone a `Balance` since that in itself would be unsafe. This operation should be safe
         * because rust guarantees that 2 instances of the same type *will* have the same memory
         * layout. Unlike 2 types which have the same fields in the same order, those are not
         * guaranteed to have the same memory layout. So this should be safe, I think...
         */
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": guild_id.to_string(),
            "UserId": user_id.to_string(),
            "CurrName": curr_name.clone(),
        };
        let res = coll.find_one(filterdoc, None).await?;
        if res.is_some() {
            return Err(anyhow!("User already has a balance for that currency in that guild."));
        }

        coll.insert_one(user_balance2, None).await?;
        Ok(user_balance)
    }

    /// Attempts to get the currency corresponding to this balance.
    /// # Errors
    /// - Any `MongoDB` error occurs.
    /// - The currency does not exist.
    pub async fn currency(&self) -> Result<Option<ArcTokioMutexOption<Currency>>> {
        Currency::try_from_name(self.guild_id.clone(), self.curr_name.clone()).await
    }

    #[allow(clippy::must_use_candidate)]
    pub fn guild_id(&self) -> &DbGuildId {
        &self.guild_id
    }

    #[allow(clippy::must_use_candidate)]
    pub fn user_id(&self) -> &DbUserId {
        &self.user_id
    }

    #[allow(clippy::must_use_candidate)]
    pub fn curr_name(&self) -> &str {
        &self.curr_name
    }

    #[allow(clippy::must_use_candidate)]
    pub fn amount(&self) -> f64 {
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
    pub async fn set_amount(&mut self, mut amount: f64) -> Result<()> {
        if amount.is_infinite() {
            return Err(anyhow!("Amount cannot be infinite."));
        }
        if amount.is_nan() {
            return Err(anyhow!("Amount cannot be NaN."));
        }

        self.set_amount_unchecked(amount).await
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
    pub async fn add_amount(&mut self, mut amount: f64) -> Result<()> {
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
        let new_amount = (self.amount * 100.0 + amount * 100.0).round() * 0.01;
        if new_amount.is_infinite() {
            return Err(anyhow!("Cannot add that amount, would overflow to infinity."));
        } else if new_amount.is_nan() {
            return Err(anyhow!("Cannot add that amount, would cause a NaN."));
        }
        self.set_amount(new_amount).await
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
    pub async fn sub_amount(&mut self, mut amount: f64) -> Result<()> {
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
        let new_amount = (self.amount * 100.0 - amount * 100.0).round() * 0.01;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot subtract that amount, would cause a NaN."));
        }
        self.set_amount(new_amount).await
    }

    /// Subtracts the specified amount from the current amount without checking if the balance
    /// will go into the negatives and without checking if the amount is negative. However, it still
    /// checks to see if the result of the operations turns out to be NaN.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount would cause a NaN.
    pub async fn sub_amount_unchecked(&mut self, mut amount: f64) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot subtract NaN."));
        }
        amount = (amount * 100.0).round() / 100.0;
        let new_amount = (self.amount * 100.0 - amount * 100.0).round() / 100.0;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot subtract that amount, would cause a NaN."));
        }
        self.set_amount(new_amount).await
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
    pub async fn add_amount_unchecked(&mut self, mut amount: f64) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot add NaN."));
        }
        amount = (amount * 100.0).round() / 100.0;
        let new_amount = (self.amount * 100.0 + amount * 100.0).round() / 100.0;
        if new_amount.is_nan() {
            return Err(anyhow!("Cannot add that amount, would cause a NaN."));
        }
        self.set_amount(new_amount).await
    }

    /// Sets the amount to the specified amount without checking if the amount is infinite.
    /// However, it still checks to see if the amount is NaN.
    ///
    /// # Errors
    /// - If any `MongoDB` error occurs.
    /// - If the amount of modified documents is 0.
    /// - The specified amount is NaN.
    pub async fn set_amount_unchecked(&mut self, mut amount: f64) -> Result<()> {
        if amount.is_nan() {
            return Err(anyhow!("Cannot set NaN."));
        }
        amount = (amount * 100.0).round() / 100.0;
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "UserId": self.user_id.to_string(),
            "CurrName": self.curr_name.clone(),
        };
        let updatedoc =
            doc! {
            "$set": {
                "Amount": amount,
            },
        };

        let res = coll.update_one(filterdoc, updatedoc, None).await?;
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
    pub async fn clear(&mut self) -> Result<()> {
        self.set_amount(0.0).await
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
                ::try_from_name(self.guild_id.clone(), self.curr_name.clone()).await?
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
        let mut db = super::super::CLIENT.get().await.database("conebot");
        let coll: Collection<Balance> = db.collection("balances");

        let filterdoc =
            doc! {
            "GuildId": self.guild_id.to_string(),
            "UserId": self.user_id.to_string(),
            "CurrName": self.curr_name.clone(),
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
        let user = crate::db::id::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::id::DbGuildId::from(TEST_GUILD_ID);
        let mut balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let mut balances = balances.as_mut().unwrap();
        assert_eq!(balances.balances.len(), 2); // There are 2 test currencies in the DB matching the IDs
    }

    #[tokio::test]
    #[allow(clippy::float_cmp)]
    async fn test_checked_amount_operations() {
        crate::init_env().await;
        let user = crate::db::id::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::id::DbGuildId::from(TEST_GUILD_ID);
        let mut balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let mut balances = balances.as_mut().unwrap();
        let mut balance = balances.balances
            .iter_mut()
            .find(|b| b.curr_name == "test")
            .unwrap();
        let error_margin: f64 = f64::EPSILON;

        assert!((balance.amount - 30.0).abs() < error_margin); // value in DB is 30.0
        balance.add_amount(1.0).await.unwrap();
        assert!((balance.amount - 31.0).abs() < error_margin);
        balance.sub_amount(1.0).await.unwrap();
        assert!((balance.amount - 30.0).abs() < error_margin);

        assert!(balance.add_amount(f64::INFINITY).await.is_err()); // inf check
        assert!(balance.sub_amount(f64::INFINITY).await.is_err());

        assert!(balance.add_amount(f64::MAX).await.is_err()); // overflow check
        assert!(balance.sub_amount(32.0).await.is_err());

        assert!(balance.add_amount(-1.0).await.is_err()); // negative check
        assert!(balance.sub_amount(-1.0).await.is_err());

        assert!(balance.add_amount(f64::NAN).await.is_err()); // NaN check
        assert!(balance.sub_amount(f64::NAN).await.is_err());

        balance.set_amount(0.1).await.unwrap(); // Rounding to 2dp check.
        balance.add_amount(0.2).await.unwrap(); // Precision cmp is required here without error margin.
        assert_eq!(balance.amount, 0.3);
        balance.sub_amount(0.2).await.unwrap();
        assert_eq!(balance.amount, 0.1);

        balance.set_amount(0.111).await.unwrap(); // Rounding to 2dp check part 2.
        assert_eq!(balance.amount, 0.11);
        balance.add_amount(0.222).await.unwrap();
        assert_eq!(balance.amount, 0.33);
        balance.sub_amount(0.222).await.unwrap();
        assert_eq!(balance.amount, 0.11);

        balance.set_amount(30.0).await.unwrap(); // Reset amount
    }

    #[tokio::test]
    async fn test_unchecked_amount_operations() {
        crate::init_env().await;
        let user = crate::db::id::DbUserId::from(TEST_USER_ID);
        let guild = crate::db::id::DbGuildId::from(TEST_GUILD_ID);
        let mut balances = super::Balances::try_from_user(guild, user).await.unwrap();
        let mut balances = balances.lock().await;
        let mut balances = balances.as_mut().unwrap();
        let mut balance = balances.balances
            .iter_mut()
            .find(|b| b.curr_name == "test")
            .unwrap();
        let error_margin: f64 = f64::EPSILON;
        assert!(balance.amount - 30.0 < error_margin);

        balance.add_amount_unchecked(f64::INFINITY).await.unwrap();
        assert!(balance.amount.is_infinite() && balance.amount.is_sign_positive());
        balance.set_amount(30.0).await.unwrap();
        balance.sub_amount_unchecked(f64::INFINITY).await.unwrap();
        assert!(balance.amount.is_infinite() && balance.amount.is_sign_negative());
        balance.set_amount(30.0).await.unwrap();

        balance.add_amount_unchecked(-1.0).await.unwrap();
        assert!(balance.amount - 29.0 < error_margin);
        balance.sub_amount_unchecked(-1.0).await.unwrap();
        assert!(balance.amount - 30.0 < error_margin);

        assert!(balance.add_amount_unchecked(f64::NAN).await.is_err()); // NaN check
        assert!(balance.sub_amount_unchecked(f64::NAN).await.is_err());
    }
}
