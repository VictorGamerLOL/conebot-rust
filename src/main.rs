#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(clippy::await_holding_lock)]

pub mod commands;
pub mod db;
pub mod event_handler;

use dotenv::dotenv;
use serenity::{prelude::GatewayIntents, Client};
use std::env;

#[tokio::main]
async fn main() {
    init_env().await;

    let token = match env::var("TOKEN") {
        Ok(token) => token,
        Err(e) => panic!("Error: {}", e),
    };

    let mut client = Client::builder(token, GatewayIntents::all())
        .event_handler(event_handler::Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        eprintln!("Client error: {:#?}", why);
    }
}

async fn init_env() {
    dotenv().ok();
    db::init().await;
}
