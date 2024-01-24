use anyhow::Result;
use mongodb::ClientSession;

use crate::db::{ models::{ Balances, DropTable, Item }, uniques::DbGuildId };

pub async fn handle_name_updates(
    guild_id: DbGuildId,
    before: &str,
    after: String,
    session: &mut ClientSession
) -> Result<()> {
    Balances::bulk_update_currency_name(guild_id, before, after.clone(), Some(session)).await?;
    DropTable::bulk_update_part_currency_name(
        guild_id,
        before,
        after.clone(),
        Some(session)
    ).await?;
    Item::bulk_update_currency_value_name(guild_id, before, after, Some(session)).await?;
    // TODO: The rest
    Ok(())
}
