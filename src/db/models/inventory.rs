pub struct Inventory {
    guild_id: String,
    user_id: String,
    inventory: Vec<InventoryEntry>,
}
pub struct InventoryEntry {
    guild_id: String,
    user_id: String,
    item_name: String,
    amount: i64, // Should not go into negatives. Enforce at runtime.
}
