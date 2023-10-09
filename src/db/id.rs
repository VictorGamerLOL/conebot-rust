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
use serde::{Deserialize, Serialize};
use serenity::model::prelude::{ChannelId, GuildId, RoleId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbGuildId(pub String);

impl From<u64> for DbGuildId {
    fn from(id: u64) -> Self {
        Self(id.to_string())
    }
}

impl TryFrom<DbGuildId> for u64 {
    type Error = anyhow::Error;
    fn try_from(id: DbGuildId) -> Result<Self> {
        Ok(id.0.parse()?)
    }
}

impl From<GuildId> for DbGuildId {
    fn from(id: GuildId) -> Self {
        Self(id.0.to_string())
    }
}

impl TryFrom<DbGuildId> for GuildId {
    type Error = anyhow::Error;
    fn try_from(id: DbGuildId) -> Result<Self> {
        Ok(Self(id.0.parse()?))
    }
}

impl ToString for DbGuildId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}
impl From<DbGuildId> for String {
    fn from(id: DbGuildId) -> Self {
        id.0
    }
}

impl From<String> for DbGuildId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for DbGuildId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl AsRef<str> for DbGuildId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Default for DbGuildId {
    fn default() -> Self {
        Self("0".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbUserId(pub String);

impl From<u64> for DbUserId {
    fn from(id: u64) -> Self {
        Self(id.to_string())
    }
}

impl TryFrom<DbUserId> for u64 {
    type Error = anyhow::Error;
    fn try_from(id: DbUserId) -> Result<Self> {
        Ok(id.0.parse()?)
    }
}

impl From<UserId> for DbUserId {
    fn from(id: UserId) -> Self {
        Self(id.0.to_string())
    }
}

impl TryFrom<DbUserId> for UserId {
    type Error = anyhow::Error;
    fn try_from(id: DbUserId) -> Result<Self> {
        Ok(Self(id.0.parse()?))
    }
}

impl ToString for DbUserId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<DbUserId> for String {
    fn from(id: DbUserId) -> Self {
        id.0
    }
}

impl From<String> for DbUserId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for DbUserId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl AsRef<str> for DbUserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Default for DbUserId {
    fn default() -> Self {
        Self("0".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbChannelId(pub String);

impl From<u64> for DbChannelId {
    fn from(id: u64) -> Self {
        Self(id.to_string())
    }
}

impl TryFrom<DbChannelId> for u64 {
    type Error = anyhow::Error;
    fn try_from(id: DbChannelId) -> Result<Self> {
        Ok(id.0.parse()?)
    }
}

impl From<ChannelId> for DbChannelId {
    fn from(id: ChannelId) -> Self {
        Self(id.0.to_string())
    }
}

impl TryFrom<DbChannelId> for ChannelId {
    type Error = anyhow::Error;
    fn try_from(id: DbChannelId) -> Result<Self> {
        Ok(Self(id.0.parse()?))
    }
}

impl ToString for DbChannelId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<DbChannelId> for String {
    fn from(id: DbChannelId) -> Self {
        id.0
    }
}

impl From<String> for DbChannelId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for DbChannelId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl AsRef<str> for DbChannelId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Default for DbChannelId {
    fn default() -> Self {
        Self("0".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "PascalCase"))]
pub struct DbRoleId(pub String);

impl From<u64> for DbRoleId {
    fn from(id: u64) -> Self {
        Self(id.to_string())
    }
}

impl TryFrom<DbRoleId> for u64 {
    type Error = anyhow::Error;
    fn try_from(id: DbRoleId) -> Result<Self> {
        Ok(id.0.parse()?)
    }
}

impl From<RoleId> for DbRoleId {
    fn from(id: RoleId) -> Self {
        Self(id.0.to_string())
    }
}

impl TryFrom<DbRoleId> for RoleId {
    type Error = anyhow::Error;
    fn try_from(id: DbRoleId) -> Result<Self> {
        Ok(Self(id.0.parse()?))
    }
}

impl ToString for DbRoleId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<DbRoleId> for String {
    fn from(id: DbRoleId) -> Self {
        id.0
    }
}

impl From<String> for DbRoleId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for DbRoleId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl AsRef<str> for DbRoleId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Default for DbRoleId {
    fn default() -> Self {
        Self("0".into())
    }
}
