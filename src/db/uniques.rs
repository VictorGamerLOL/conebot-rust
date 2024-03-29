//! This module contains the types for the IDs / snowflakes used in the database.
//!
//! The term ID and snowflake may be used interchangeably.
//!
//! The IDs are wrapped in structs for added safety and to make them easier to work with.
//!
//! IDs are represented in strings because of BSON document limitations. A BSON document only supports the following number types:
//!
//! - 64-bit binary floating point
//! - 32-bit signed integer
//! - 64-bit signed integer
//! - 128-bit decimal floating point
//!
//! Since Rust does not have support for decimal128, the only option is to use i64.
//! However, i64 is not large enough to store a snowflake from Serenity, which is a u64.
//! Therefore the snowflakes have their bytes interpreted as an i64 and stored in the
//! structs as that. The bytes are then interpreted as a u64 when needed. Since the functions
//! that do this are const, this is safe and very performant as it does not require any
//! allocation or conversion. The functions claim that this is no-op so it should have
//! little to no impact.
//!
//! The fact that `MongoDB` uses `BigEndian` and the average `x86_64` CPU uses `LittleEndian`
//! is trivial because the bson serializer and deserializer will handle that for us.

use std::borrow::Cow;

use anyhow::Result;
use serde::{ Deserialize, Serialize };
use serenity::model::prelude::{ ChannelId, GuildId, RoleId, UserId };

use crate::{ db::models::Currency, macros::const_impl };

use super::models::DropTable;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
/// A wrapper around a guild ID as it should be stored in the database.
pub struct DbGuildId(i64);

impl DbGuildId {
    /// Returns the ID as an i64.
    pub const fn as_i64(self) -> i64 {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        u64::from_ne_bytes(self.0.to_ne_bytes())
    }
}

const_impl! {
    impl From<u64> for DbGuildId {
        fn from(id: u64) -> Self {
            Self(i64::from_ne_bytes(id.to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<i64> for DbGuildId {
        fn from(id: i64) -> Self {
            Self(id)
        }
    }
}

const_impl! {
    impl From<i32> for DbGuildId {
        fn from(id: i32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u32> for DbGuildId {
        fn from(id: u32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i16> for DbGuildId {
        fn from(id: i16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u16> for DbGuildId {
        fn from(id: u16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i8> for DbGuildId {
        fn from(id: i8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u8> for DbGuildId {
        fn from(id: u8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<DbGuildId> for u64 {
        fn from(id: DbGuildId) -> Self {
            Self::from_ne_bytes(id.0.to_ne_bytes())
        }
    }
}

const_impl! {
    impl From<DbGuildId> for i64 {
        fn from(id: DbGuildId) -> Self {
            id.0
        }
    }
}

const_impl! {
    impl From<GuildId> for DbGuildId {
        fn from(id: GuildId) -> Self {
            Self(i64::from_ne_bytes(id.get().to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbGuildId> for GuildId {
        fn from(id: DbGuildId) -> Self {
            Self::from(u64::from_ne_bytes(id.0.to_ne_bytes()))
        }
    }
}

impl ToString for DbGuildId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbUserId(i64);

impl DbUserId {
    pub const fn as_i64(self) -> i64 {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        u64::from_ne_bytes(self.0.to_ne_bytes())
    }
}

const_impl! {
    impl From<u64> for DbUserId {
        fn from(id: u64) -> Self {
            Self(i64::from_ne_bytes(id.to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbUserId> for u64 {
        fn from(id: DbUserId) -> Self {
            Self::from_ne_bytes(id.0.to_ne_bytes())
        }
    }
}

const_impl! {
    impl From<i64> for DbUserId {
        fn from(id: i64) -> Self {
            Self(id)
        }
    }
}

const_impl! {
    impl From<i32> for DbUserId {
        fn from(id: i32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u32> for DbUserId {
        fn from(id: u32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i16> for DbUserId {
        fn from(id: i16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u16> for DbUserId {
        fn from(id: u16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i8> for DbUserId {
        fn from(id: i8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u8> for DbUserId {
        fn from(id: u8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<UserId> for DbUserId {
        fn from(id: UserId) -> Self {
            Self(i64::from_ne_bytes(id.get().to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbUserId> for UserId {
        fn from(id: DbUserId) -> Self {
            Self::from(u64::from_ne_bytes(id.0.to_ne_bytes()))
        }
    }
}

impl ToString for DbUserId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbChannelId(i64);

impl DbChannelId {
    pub const fn as_i64(self) -> i64 {
        self.0
    }
    pub const fn as_u64(self) -> u64 {
        u64::from_ne_bytes(self.0.to_ne_bytes())
    }
}

const_impl! {
    impl From<u64> for DbChannelId {
        fn from(id: u64) -> Self {
            Self(i64::from_ne_bytes(id.to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbChannelId> for u64 {
        fn from(id: DbChannelId) -> Self {
            Self::from_ne_bytes(id.0.to_ne_bytes())
        }
    }
}

const_impl! {
    impl From<DbChannelId> for i64 {
        fn from(id: DbChannelId) -> Self {
            id.0
        }
    }
}

const_impl! {
    impl From<i64> for DbChannelId {
        fn from(id: i64) -> Self {
            Self(id)
        }
    }
}

const_impl! {
    impl From<i32> for DbChannelId {
        fn from(id: i32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u32> for DbChannelId {
        fn from(id: u32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i16> for DbChannelId {
        fn from(id: i16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u16> for DbChannelId {
        fn from(id: u16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i8> for DbChannelId {
        fn from(id: i8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u8> for DbChannelId {
        fn from(id: u8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<ChannelId> for DbChannelId {
        fn from(id: ChannelId) -> Self {
            Self(i64::from_ne_bytes(id.get().to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbChannelId> for ChannelId {
        fn from(id: DbChannelId) -> Self {
            Self::from(u64::from_ne_bytes(id.0.to_ne_bytes()))
        }
    }
}

impl ToString for DbChannelId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<String> for DbChannelId {
    type Error = anyhow::Error;
    fn try_from(id: String) -> Result<Self> {
        Ok(Self(i64::from_ne_bytes(id.parse::<u64>()?.to_ne_bytes())))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbRoleId(i64);

impl DbRoleId {
    pub const fn as_i64(self) -> i64 {
        self.0
    }
    pub const fn as_u64(self) -> u64 {
        u64::from_ne_bytes(self.0.to_ne_bytes())
    }
}

const_impl! {
    impl From<u64> for DbRoleId {
        fn from(id: u64) -> Self {
            Self(i64::from_ne_bytes(id.to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbRoleId> for u64 {
        fn from(id: DbRoleId) -> Self {
            Self::from_ne_bytes(id.0.to_ne_bytes())
        }
    }
}

const_impl! {
    impl From<i64> for DbRoleId {
        fn from(id: i64) -> Self {
            Self(id)
        }
    }
}

const_impl! {
    impl From<i32> for DbRoleId {
        fn from(id: i32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u32> for DbRoleId {
        fn from(id: u32) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i16> for DbRoleId {
        fn from(id: i16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u16> for DbRoleId {
        fn from(id: u16) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<i8> for DbRoleId {
        fn from(id: i8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<u8> for DbRoleId {
        fn from(id: u8) -> Self {
            Self(i64::from(id))
        }
    }
}

const_impl! {
    impl From<DbRoleId> for i64 {
        fn from(id: DbRoleId) -> Self {
            id.0
        }
    }
}

const_impl! {
    impl From<RoleId> for DbRoleId {
        fn from(id: RoleId) -> Self {
            Self(i64::from_ne_bytes(id.get().to_ne_bytes()))
        }
    }
}

const_impl! {
    impl From<DbRoleId> for RoleId {
        fn from(id: DbRoleId) -> Self {
            Self::from(u64::from_ne_bytes(id.0.to_ne_bytes()))
        }
    }
}

impl ToString for DbRoleId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl TryFrom<String> for DbRoleId {
    type Error = anyhow::Error;
    fn try_from(id: String) -> Result<Self> {
        Ok(Self(i64::from_ne_bytes(id.parse::<u64>()?.to_ne_bytes())))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CurrencyName(#[serde(skip)] DbGuildId, String);

impl CurrencyName {
    pub const fn db_guild_id(&self) -> DbGuildId {
        self.0
    }

    pub fn as_str(&self) -> &str {
        &self.1
    }

    pub async fn validate(&self) -> Result<bool> {
        Ok(Currency::try_from_name(self.0, self.1.clone()).await?.is_some())
    }

    pub async fn from_string_and_guild_id(guild_id: DbGuildId, name: String) -> Result<Self> {
        let tmp = Self(guild_id, name);
        if tmp.validate().await? {
            Ok(tmp)
        } else {
            anyhow::bail!("Currency does not exist.")
        }
    }

    pub fn into_string(self) -> String {
        self.1
    }

    pub fn as_ref(&self) -> CurrencyNameRef<'_> {
        CurrencyNameRef(self.0, &self.1)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CurrencyNameRef<'a>(#[serde(skip)] DbGuildId, &'a str);

impl<'a> CurrencyNameRef<'a> {
    pub const fn db_guild_id(&self) -> DbGuildId {
        self.0
    }

    pub const fn as_str(&self) -> &str {
        self.1
    }

    pub const fn from_str_and_guild_id_unchecked(guild_id: DbGuildId, name: &'a str) -> Self {
        Self(guild_id, name)
    }

    pub fn to_owned(&self) -> CurrencyName {
        CurrencyName(self.0, self.1.to_owned())
    }
}
impl PartialEq<&str> for CurrencyNameRef<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.1 == *other
    }
}

impl PartialEq<CurrencyNameRef<'_>> for &str {
    fn eq(&self, other: &CurrencyNameRef<'_>) -> bool {
        *self == other.1
    }
}

impl PartialEq<String> for CurrencyNameRef<'_> {
    fn eq(&self, other: &String) -> bool {
        self.1 == other
    }
}

impl PartialEq<CurrencyNameRef<'_>> for String {
    fn eq(&self, other: &CurrencyNameRef<'_>) -> bool {
        self == other.1
    }
}

#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DropTableName(#[serde(skip)] DbGuildId, String);

impl DropTableName {
    pub const fn db_guild_id(&self) -> DbGuildId {
        self.0
    }

    pub fn as_str(&self) -> &str {
        &self.1
    }

    pub async fn validate(&self) -> Result<bool> {
        DropTable::try_from_name(self.db_guild_id(), Cow::Borrowed(self.as_str()), None).await?;
        Ok(true)
    }

    pub async fn from_string_and_guild_id(guild_id: DbGuildId, name: String) -> Result<Self> {
        let tmp = Self(guild_id, name);
        if tmp.validate().await? {
            Ok(tmp)
        } else {
            anyhow::bail!("Invalid drop table name.")
        }
    }

    /// # Safety
    /// This function is unsafe because it does not check if the drop table name exists in the
    /// database. It is up to the caller to ensure that the drop table name exists.
    pub const unsafe fn from_string_and_guild_id_unchecked(
        guild_id: DbGuildId,
        name: String
    ) -> Self {
        Self(guild_id, name)
    }

    /// The actual `AsRef` trait is not implemented because
    /// "`Borrow` (and `AsRef`) can only return _references to something that already exists_" and
    /// "a `FooRef` is a new thing; there is no preexisting memory that has a `FooRef` in it"
    pub const fn as_ref(&self) -> DropTableNameRef<'_> {
        DropTableNameRef(self.0, &self.1)
    }

    pub fn into_string(self) -> String {
        self.1
    }
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DropTableNameRef<'a>(DbGuildId, &'a String);

impl<'a> DropTableNameRef<'a> {
    pub const fn db_guild_id(&self) -> DbGuildId {
        self.0
    }

    pub const fn as_str(&self) -> &String {
        self.1
    }

    pub async fn validate(&self) -> Result<bool> {
        DropTable::try_from_name(self.0, Cow::Borrowed(self.1), None).await?;
        Ok(true)
    }

    /// # Safety
    /// This function is unsafe because it does not check if the drop table name exists in the
    /// database. It is up to the caller to ensure that the drop table name exists.
    pub const unsafe fn from_string_ref_and_guild_id_unchecked(
        guild_id: DbGuildId,
        name: &'a String
    ) -> Self {
        Self(guild_id, name)
    }

    /// The actual `ToOwned` trait is not implemented because
    /// "`Borrow` (and `AsRef`) can only return _references to something that already exists_" and
    /// "a `FooRef` is a new thing; there is no preexisting memory that has a `FooRef` in it"
    pub fn to_owned(&self) -> DropTableName {
        DropTableName(self.0, self.1.to_owned())
    }
}
