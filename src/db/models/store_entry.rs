pub struct StoreEntry {
    guild_id: String,
    item_name: String,
    curr_name: String,
    value: f64,
    amount: i64, // Neither this or stock_amount should go into negatives.
    stock_amount: i64, // Enforce at runtime.
    expiry_date: chrono::DateTime<chrono::Utc>,
    role_restrictions: Vec<String>,
}
