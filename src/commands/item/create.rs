use std::time::Duration;

use anyhow::{ anyhow, bail, Result };
use serenity::{
    builder::CreateApplicationCommandOption,
    collector::CollectReply,
    http::{ CacheHttp, Http },
    model::prelude::{
        application_command::ApplicationCommandInteraction,
        command::CommandOptionType,
        UserId,
    },
    prelude::Context,
};

use crate::event_handler::command_handler::CommandOptions;

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: Context
) -> Result<()> {
    let command_author = command.user.id;
    Ok(())
}

const CREATE_OPTION_NAME: &str = "create";
const NAME_OPTION_NAME: &str = "name";
const DESCRIPTION_OPTION_NAME: &str = "description";
const SELLABLE_OPTION_NAME: &str = "sellable";
const TRADEABLE_OPTION_NAME: &str = "tradeable";
const CURRENCY_OPTION_NAME: &str = "currency";
const VALUE_OPTION_NAME: &str = "value";

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("create")
        .kind(CommandOptionType::SubCommand)
        .description("Create a new item.")
        .create_sub_option(|option| {
            option
                .name("name")
                .description("The name of the item, cannot be blank.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|option| {
            option
                .name("description")
                .description("The description of the item.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name("sellable")
                .description("Whether the item can be sold for the currency it corresponds to.")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name("tradeable")
                .description("Whether the item can be traded between users.")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name("currency")
                .description("The currency the item corresponds to.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|option| {
            option
                .name("value")
                .description("The value of the item in terms of the currency it corresponds to.")
                .kind(CommandOptionType::Number)
                .required(true)
        })
        .create_sub_option(|option| {
            option
                .name("type")
                .description("How the item behaves.")
                .kind(CommandOptionType::String)
                .required(false)
                .add_string_choice("Trophy", "Trophy")
                .add_string_choice("Consumable", "Consumable")
                .add_string_choice("InstantConsumable", "InstantCOnsumable")
        })
        .create_sub_option(|option| {
            option
                .name("message")
                .description("The message to send when the item is used. Ignored when trophy.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name("action_type")
                .description(
                    "The type of action to perform when the item is used. Ignored when trophy."
                )
                .kind(CommandOptionType::String)
                .required(false)
                .add_string_choice("None", "None")
                .add_string_choice("Role", "Role")
                .add_string_choice("Lootbox", "Lootbox")
        })
        .create_sub_option(|option| {
            option
                .name("role")
                .description(
                    "The role to give when the item is used. Ignored when trophy or action_type is None or Lootbox."
                )
                .kind(CommandOptionType::Role)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name("drop_table")
                .description(
                    "The drop table to use when the item is used. Ignored when trophy or action_type is None or Role."
                )
                .kind(CommandOptionType::String)
                .required(false)
        });
    option
}
