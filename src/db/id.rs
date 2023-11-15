//! This module contains the types for the IDs / snowflakes used in the database.
//!
//! The term ID and snowflake may be used interchangeably.
//!
//! The IDs are wrapped in structs for added safety and to make them easier to work with.
//!
//! IDs are represented in strings because of BSON document limitations. A BSON document only supports the following number types:
//!
//! - 64-bit decimal floating point
//! - 32-bit signed integer
//! - 64-bit signed integer
//! - 128-bit decimal floating point
//!
//! Since Rust does not have support for decimal128, the only option is to use strings.
//! This is because if I were to give the BSON serializer a u64, it would try to
//! convert it to a signed 64-bit integer, which would possibly result in an overflow. So
//! it is better to instead take a hit on the amount of space used in the database rather
//! than risk an overflow.
//!
//! I understand why serenity stores snowflakes as u64 to save on RAM, but it is mildly annoying
//! to work with.
//!
//! These structs contain the necessary methods to convert them to strings, u64s, serenity types and vice versa.

use anyhow::Result;
use serde::{ Deserialize, Serialize };
use serenity::model::prelude::{ ChannelId, GuildId, RoleId, UserId };

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbGuildId(i64);

impl DbGuildId {
    pub const fn as_i64(self) -> i64 {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        u64::from_ne_bytes(self.0.to_ne_bytes())
    }
}

impl From<u64> for DbGuildId {
    fn from(id: u64) -> Self {
        Self(i64::from_ne_bytes(id.to_ne_bytes()))
    }
}

impl From<DbGuildId> for u64 {
    fn from(id: DbGuildId) -> Self {
        Self::from_ne_bytes(id.0.to_ne_bytes())
    }
}

impl From<GuildId> for DbGuildId {
    fn from(id: GuildId) -> Self {
        Self(i64::from_ne_bytes(id.0.to_ne_bytes()))
    }
}

impl TryFrom<DbGuildId> for GuildId {
    type Error = anyhow::Error;
    fn try_from(id: DbGuildId) -> Result<Self> {
        Ok(Self(u64::from_ne_bytes(id.0.to_ne_bytes())))
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

impl From<u64> for DbUserId {
    fn from(id: u64) -> Self {
        Self(i64::from_ne_bytes(id.to_ne_bytes()))
    }
}

impl From<DbUserId> for u64 {
    fn from(id: DbUserId) -> Self {
        Self::from_ne_bytes(id.0.to_ne_bytes())
    }
}

impl From<UserId> for DbUserId {
    fn from(id: UserId) -> Self {
        Self(i64::from_ne_bytes(id.0.to_ne_bytes()))
    }
}

impl From<DbUserId> for UserId {
    fn from(id: DbUserId) -> Self {
        Self(u64::from_ne_bytes(id.0.to_ne_bytes()))
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

impl From<u64> for DbChannelId {
    fn from(id: u64) -> Self {
        Self(i64::from_ne_bytes(id.to_ne_bytes()))
    }
}

impl From<DbChannelId> for u64 {
    fn from(id: DbChannelId) -> Self {
        Self::from_ne_bytes(id.0.to_ne_bytes())
    }
}

impl From<DbChannelId> for i64 {
    fn from(id: DbChannelId) -> Self {
        id.0
    }
}

impl From<ChannelId> for DbChannelId {
    fn from(id: ChannelId) -> Self {
        Self(i64::from_ne_bytes(id.0.to_ne_bytes()))
    }
}

impl From<DbChannelId> for ChannelId {
    fn from(id: DbChannelId) -> Self {
        Self(u64::from_ne_bytes(id.0.to_ne_bytes()))
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

impl From<u64> for DbRoleId {
    fn from(id: u64) -> Self {
        Self(i64::from_ne_bytes(id.to_ne_bytes()))
    }
}

impl From<DbRoleId> for u64 {
    fn from(id: DbRoleId) -> Self {
        Self::from_ne_bytes(id.0.to_ne_bytes())
    }
}

impl From<RoleId> for DbRoleId {
    fn from(id: RoleId) -> Self {
        Self(i64::from_ne_bytes(id.0.to_ne_bytes()))
    }
}

impl From<DbRoleId> for RoleId {
    fn from(id: DbRoleId) -> Self {
        Self(u64::from_ne_bytes(id.0.to_ne_bytes()))
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
