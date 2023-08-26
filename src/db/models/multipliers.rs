pub struct Multipliers {
    guild_id: String,
    user_id: String,
    multipliers: Vec<Multiplier>,
}
pub struct Multiplier {
    guild_id: String,
    user_id: String,
    curr_name: String,
    multiplier: f64,
    expiry_date: chrono::DateTime<chrono::Utc>,
}
