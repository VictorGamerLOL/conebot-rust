use std::borrow::Cow;

use anyhow::{ anyhow, Result };
use lazy_static::lazy_static;
use mongodb::ClientSession;
use serenity::{ all::{ GuildId, Mention, RoleId, UserId }, http::{ CacheHttp, Http } };

use crate::{
    db::{
        models::{
            item::{ ItemActionType, ItemType },
            Balances,
            DropTable,
            Inventory,
            InventoryEntry,
            Item,
        },
        CLIENT,
    },
    mechanics::drop_generator::DropGenerator,
};

use super::drop_generator::{ DropResult, DropResultKind };

lazy_static! {
    static ref REPEAT_PATTERN_REGEX: regex::Regex = regex::Regex::new(r"%%(.*)%%").unwrap();
}

//TODO: Fill this in with drop tables once they are implemented.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UseResult<'a> {
    pub success: bool,
    pub message: Option<Cow<'a, str>>,
    pub content: UseResultContent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum UseResultContent {
    RoleAdd(RoleId),
    DropGiven(Vec<(String, i64)>),
    Nothing,
}

/// # Warning
/// This function will attempt to lock the user's balances. It ***WILL***
/// cause a deadlock if the balances are already locked before this function
/// is called.
pub async fn use_item<'a>(
    user: UserId,
    user_inv: &mut Inventory,
    item: &'a Item,
    times: i64,
    http: impl AsRef<Http> + CacheHttp + Send + Sync + Clone
) -> Result<UseResult<'a>> {
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
            // if the user uses a role item multiple times it's their own fault.
            give_role(user, (*role_id).into(), item.guild_id().into(), http).await?;
            Ok(UseResult {
                success: true,
                message: Some(
                    Cow::Owned({
                        let msg_with_count = message
                            .unwrap_or("You were given the role {{ROLE}}")
                            .replace(
                                "{{ROLE}}",
                                &Mention::from(RoleId::from(*role_id)).to_string()
                            );
                        if times > 1 {
                            format!("{} (x{})", msg_with_count, times)
                        } else {
                            msg_with_count
                        }
                    })
                ),
                content: UseResultContent::RoleAdd((*role_id).into()),
            })
        }
        ItemActionType::Lootbox { drop_table_name, count } => {
            let drop_table = DropTable::try_from_name(
                item.guild_id(),
                Cow::from(drop_table_name),
                None
            ).await?;
            let drop_table = drop_table.read().await;
            let drop_table_ = drop_table
                .as_ref()
                .ok_or_else(|| anyhow!("Drop table is being used in a breaking operation."))?;
            let dropper = DropGenerator::from(drop_table_);
            drop(drop_table);
            let drops = dropper.generate(*count * times)?;
            give_drops(item.guild_id().into(), user, user_inv, drops.clone()).await?;
            // extract the string that is between %% in the message
            let mut message = message
                .unwrap_or("Got %%*{{ITEM_CURRENCY_NAME}}*x{{AMOUNT}} %%")
                .to_owned();
            if message.is_empty() {
                message = "Got %%*{{ITEM_CURRENCY_NAME}}*x{{AMOUNT}} %%".to_owned();
            }
            let message_cloned = message.clone();
            let repeat_pattern = REPEAT_PATTERN_REGEX.find_iter(&message_cloned);
            for match_ in repeat_pattern {
                let match_ = match_.as_str().to_owned();
                let mut repeat_string = String::new();
                for drop in drops.iter() {
                    repeat_string.push_str(
                        &match_
                            .replace("{{ITEM_CURRENCY_NAME}}", drop.name())
                            .replace("{{AMOUNT}}", &drop.quantity.to_string())
                            .replace("%%", "")
                    );
                }
                message = REPEAT_PATTERN_REGEX.replace(&message, &repeat_string).into_owned();
            }
            Ok(UseResult {
                success: true,
                message: Some(Cow::Owned(message)),
                content: UseResultContent::DropGiven(
                    drops
                        .into_iter()
                        .map(|d| (d.name().to_owned(), d.quantity))
                        .collect()
                ),
            })
        }
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

pub async fn give_drops(
    guild: GuildId,
    user: UserId,
    user_inv: &mut Inventory,
    drops: Vec<DropResult<'_>>
) -> Result<()> {
    let currency_drops = drops
        .iter()
        .copied()
        .filter(|d| matches!(d.result, DropResultKind::Currency(_)))
        .collect::<Vec<_>>();
    let item_drops = drops
        .iter()
        .copied()
        .filter(|d| matches!(d.result, DropResultKind::Item(_)))
        .collect::<Vec<_>>();

    let client = CLIENT.get().await;
    let mut session = client.start_session(None).await?;

    session.start_transaction(None).await?;
    let res: Result<()> = {
        if !currency_drops.is_empty() {
            let balances = Balances::try_from_user(guild.into(), user.into()).await?;
            let mut balances = balances.lock().await;
            let balances_ = balances
                .as_mut()
                .ok_or_else(|| {
                    anyhow!("Member's balances are being used in a breaking operation.")
                })?;
            for currency in currency_drops {
                give_currency(currency, balances_, &mut session).await?;
            }
            drop(balances);
        }
        if !item_drops.is_empty() {
            for item in item_drops {
                give_items(item, user_inv, &mut session).await?;
            }
        }
        Ok(())
    };
    if res.is_err() {
        session.abort_transaction().await?;
        return res;
    } else {
        session.commit_transaction().await?;
    }
    Ok(())
}

pub async fn give_items(
    items: DropResult<'_>,
    inventory: &mut Inventory,
    session: &mut ClientSession
) -> Result<()> {
    if !matches!(items.result, DropResultKind::Item(_)) {
        anyhow::bail!("DropResult is not an item.");
    }
    inventory.give_item(Cow::from(items.name()), items.quantity, Some(session)).await?;
    Ok(())
}

pub async fn give_currency(
    currency: DropResult<'_>,
    balances: &mut Balances,
    session: &mut ClientSession
) -> Result<()> {
    if !matches!(currency.result, DropResultKind::Currency(_)) {
        anyhow::bail!("DropResult is not currency.");
    }
    let balance = balances.ensure_has_currency(Cow::from(currency.name())).await?;
    balance.add_amount(currency.quantity as f64, Some(session)).await?;
    Ok(())
}
