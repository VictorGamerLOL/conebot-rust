pub struct Balances {
    guild_id: String,
    user_id: String,
    balances: Vec<Balance>,
}
pub struct Balance {
    guild_id: String,
    user_id: String,
    curr_name: String,
    amount: f64,
}
