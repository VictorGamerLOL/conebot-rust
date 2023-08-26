#[allow(clippy::must_use_candidate)]
pub fn truncate_2dp(amount: f64) -> f64 {
    if (amount * 100.0).is_infinite() {
        return amount;
    }
    (amount * 100.0).trunc() / 100.0
}
