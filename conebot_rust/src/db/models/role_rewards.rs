pub struct RoleRewards {
    guild_id: String,
    role_rewards: Vec<RoleReward>,
}
pub struct RoleReward {
    guild_id: String,
    role_id: String,
    curr_name: String,
    amount: f64,
    interval: chrono::Duration,
    last_given: chrono::DateTime<chrono::Utc>,
}
