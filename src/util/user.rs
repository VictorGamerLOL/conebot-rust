use crate::db::models::{ InventoryEntry, Item, ItemError };
use anyhow::Result;

pub async fn is_hanging_item_entry(item: &InventoryEntry) -> Result<bool> {
    let item = Item::try_from_name(item.guild_id(), item.item_name().to_owned()).await;
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
