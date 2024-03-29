#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)] // and keep this off
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)] // cant be asked
#![allow(clippy::missing_errors_doc)] // cant be asked
#![allow(clippy::missing_panics_doc)] // cant be asked
#![deny(elided_lifetimes_in_paths)]
#![cfg_attr(feature = "is-nightly", feature(const_trait_impl))]

pub mod commands;
pub mod db;
pub mod event_handler;
pub mod mechanics;
pub mod util;

#[macro_use]
pub mod macros;

use dotenv::dotenv;
use serenity::{ model::gateway::GatewayIntents, Client };
use std::env;
use tracing::{ span, warn };
use tracing_subscriber::{ fmt, fmt::format, EnvFilter };

const ACCENT_COLOUR: u32 = 0x0003_75b4;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::from_default_env();
    let subscriber = fmt().event_format(format().pretty()).with_env_filter(filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
    span!(tracing::Level::TRACE, "main");
    init_env().await;

    if cfg!(feature = "is-nightly") {
        warn!("Rust nightly detected. Enabling nightly exclusive features.");
    }

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
