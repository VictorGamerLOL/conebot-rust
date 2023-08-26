#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(clippy::await_holding_lock)]
// #![warn(clippy::pedantic)] // TODO Enable this when finishing code.
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)] // cant be asked
#![deny(elided_lifetimes_in_paths)]

pub mod commands;
pub mod db;
pub mod event_handler;
pub mod util;

use dotenv::dotenv;
use serenity::{ prelude::GatewayIntents, Client };
use std::env;
use tracing::{ debug, error, info, span, trace, warn };
use tracing_subscriber::{ fmt, fmt::format, EnvFilter };

#[tokio::main]
async fn main() {
    let filter = EnvFilter::from_default_env();
    let subscriber = fmt().event_format(format().pretty()).with_env_filter(filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
    let sp = span!(tracing::Level::TRACE, "main");
    let g = sp.enter();
    init_env().await;
    error!("test");
    warn!("test");
    info!("test");
    debug!("test");
    trace!("test");

    let token = match env::var("TOKEN") {
        Ok(token) => token,
        Err(e) => panic!("Error: {e}"),
    };

    let mut client = Client::builder(token, GatewayIntents::all())
        .event_handler(event_handler::Handler).await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        eprintln!("Client error: {why:#?}");
    }
}

async fn init_env() {
    dotenv().ok();
    db::init().await;
}
