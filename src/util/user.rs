use crate::db::models::{ Balance, InventoryEntry, Item, ItemError };
use anyhow::{ anyhow, bail, Result };

pub async fn is_hanging_item_entry(item: &InventoryEntry) -> Result<bool> {
    let item = Item::try_from_name(item.guild_id().to_owned(), item.item_name().to_owned()).await;
    if let Err(item_err) = item {
        match item_err {
            ItemError::ItemNotFound => {
                return Ok(true);
            }
            ItemError::Other(e) => {
                return Err(e);
            }
        }
    }
    Ok(false)
}

pub async fn is_hanging_balance_entry(balance: &Balance) -> Result<bool> {
    let balance = Item::try_from_name(
        balance.guild_id().to_owned(),
        balance.curr_name().to_owned()
    ).await;
    Ok(false)
}
