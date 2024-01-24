use anyhow::Result;

use mongodb::ClientSession;

use crate::db::{ models::{ DropTable, Inventory }, uniques::DbGuildId };

/// Handles the name updates for items.
///
/// # Locks
/// - [`Inventory`] (write)
/// - [`DropTable`] (write)
/// - [`StoreEntry`] (write)
pub async fn handle_name_updates(
    guild_id: DbGuildId,
    before: String,
    after: String,
    session: &mut ClientSession
) -> Result<()> {
    // INVENTORIES
    Inventory::bulk_update_item_name(guild_id, &before, &after, Some(session)).await?;
    // END INVENTORIES

    // DROP TABLES
    DropTable::bulk_update_part_item_name(guild_id, &before, &after, Some(session)).await?;

    // STORE ENTRIES
    // TODO

    Ok(())
}
