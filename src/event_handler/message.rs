use crate::db::id::{ DbGuildId, DbUserId };
use anyhow::{ anyhow, Result };
use chrono::Duration;
use lazy_static::lazy_static;
use serenity::client::Context;
use serenity::model::channel::Message;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Timeout {
    pub user: DbUserId,
    pub guild: DbGuildId,
    pub currency: String,
}

lazy_static! {
    static ref TIMEOUTS: Arc<Mutex<HashSet<Timeout>>> = Arc::new(Mutex::new(HashSet::new()));
}

pub async fn message(_ctx: Context, new_message: Message) -> Result<()> {
    let user: DbUserId = new_message.author.id.into();
    let guild: DbGuildId = if let Some(g) = new_message.guild_id {
        g.into()
    } else {
        return Ok(());
    };
    let mut timeouts = TIMEOUTS.lock().await;
    let mut timeout = Timeout {
        user,
        guild,
        currency: "aa".to_string(),
    };
    if timeouts.contains(&timeout) {
        return Ok(());
    }
    info!("Adding timeout: {:?}", timeout);
    timeouts.insert(timeout.clone());

    tokio::spawn(async move {
        tokio::time::sleep(Duration::seconds(5).to_std().unwrap()).await;
        info!("Removing timeout: {:?}", timeout);
        let mut timeouts = TIMEOUTS.lock().await;
        timeouts.remove(&timeout);
    });
    Ok(())
}
