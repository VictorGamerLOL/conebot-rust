use std::borrow::Cow;

use anyhow::{ anyhow, Result };
use serenity::{ all::{ GuildId, RoleId, UserId, Mention }, http::{ CacheHttp, Http } };

use crate::db::models::{ item::{ ItemActionType, ItemType }, Item };

//TODO: Fill this in with drop tables once they are implemented.

pub struct UseResult<'a> {
    pub success: bool,
    pub message: Option<Cow<'a, str>>,
    pub content: UseResultContent,
}

#[non_exhaustive]
pub enum UseResultContent {
    RoleAdd(RoleId),
    Nothing,
}

pub async fn use_item(
    user: UserId,
    item: &Item,
    http: impl AsRef<Http> + CacheHttp + Send + Sync + Clone
) -> Result<UseResult<'_>> {
    if matches!(item.item_type(), ItemType::Trophy) {
        return Ok(UseResult {
            success: false,
            message: Some(Cow::Borrowed("You cannot use trophies.")),
            content: UseResultContent::Nothing,
        });
    }
    let action_type = item.action_type().ok_or_else(|| anyhow!("Item has no action type"))?;
    let message: Option<&str> = item.message().map(|s| s.as_str());
    match action_type {
        ItemActionType::Role { role_id } => {
            give_role(user, (*role_id).into(), item.guild_id().into(), http).await?;
            Ok(UseResult {
                success: true,
                message: Some(
                    Cow::Owned(
                        message
                            .unwrap_or("You were given the role {{ROLE}}")
                            .replace(
                                "{{ROLE}}",
                                &format!("{}", Mention::from(RoleId::from(*role_id)))
                            )
                    )
                ),
                content: UseResultContent::RoleAdd((*role_id).into()),
            })
        }
        ItemActionType::Lootbox { drop_table_name } =>
            Ok(UseResult {
                success: false,
                message: Some(Cow::Borrowed("Lootboxes are not implemented yet.")),
                content: UseResultContent::Nothing,
            }),
        ItemActionType::None =>
            Ok(UseResult {
                success: true,
                message: Some(Cow::Borrowed(message.unwrap_or("You used the item."))),
                content: UseResultContent::Nothing,
            }),
    }
}

pub async fn give_role(
    user: UserId,
    role: RoleId,
    guild: GuildId,
    http: impl AsRef<Http> + CacheHttp + Send + Sync + Clone
) -> Result<()> {
    let member = guild.member(&http, user).await?;
    member.add_role(http, role).await?;
    Ok(())
}
