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

use crate::{
    db::models::item::{
        self,
        builder::{ ActionTypeItemTypeBuilder, ItemTypeBuilder, ItemTypeTypeBuilder },
    },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let command_author = command.user.id;
    let name = options
        .get_string_value(NAME_OPTION_NAME)
        .ok_or_else(|| anyhow!("Name is required."))??;
    let description = options.get_string_value(DESCRIPTION_OPTION_NAME).transpose()?;
    let sellable = options.get_bool_value(SELLABLE_OPTION_NAME).transpose()?;
    let tradeable = options.get_bool_value(TRADEABLE_OPTION_NAME).transpose()?;
    let currency = options.get_string_value(CURRENCY_OPTION_NAME).transpose()?;
    let value = options
        .get_int_or_number_value(VALUE_OPTION_NAME)
        .transpose()?
        .map(IntOrNumber::cast_to_f64);
    let item_type = options.get_string_value(TYPE_OPTION_NAME).transpose()?;
    let message = options.get_string_value(MESSAGE_OPTION_NAME).transpose()?;
    let action_type = options.get_string_value(ACTION_TYPE_OPTION_NAME).transpose()?;
    let role = options.get_role_value(ROLE_OPTION_NAME).transpose()?;
    let drop_table = options.get_string_value(DROP_TABLE_OPTION_NAME).transpose()?;

    let mut item_builder = item::builder::Builder::new(
        command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?.into(),
        name.clone()
    );
    item_builder
        .description(description)
        .sellable(sellable)
        .tradeable(tradeable)
        .currency_value(currency)
        .value(value);
    let mut item_type_builder = item::builder::ItemTypeBuilder::new();
    if let Some(s) = item_type {
        let type_ = ItemTypeTypeBuilder::from_string(s)?;
        item_type_builder.type_(Some(type_));
    }
    if let Some(s) = message {
        item_type_builder.message(Some(s));
    }
    if let Some(s) = action_type {
        let action_type = ActionTypeItemTypeBuilder::from_string(s)?;
        item_type_builder.action_type(Some(action_type));
    }
    item_type_builder.role(role.map(|r| r.id.into())).drop_table_name(drop_table);

    let item_type = item_type_builder.build()?;

    let item_type_string = item_type.to_string();

    item_builder.item_type(Some(item_type));

    item_builder.build().await?;

    command.edit_original_interaction_response(&http, |response| {
        response.content(format!("Item `{}` of type `{}` created.", name, item_type_string))
    }).await?;

    Ok(())
}

const CREATE_OPTION_NAME: &str = "create";
const NAME_OPTION_NAME: &str = "name";
const DESCRIPTION_OPTION_NAME: &str = "description";
const SELLABLE_OPTION_NAME: &str = "sellable";
const TRADEABLE_OPTION_NAME: &str = "tradeable";
const CURRENCY_OPTION_NAME: &str = "currency";
const VALUE_OPTION_NAME: &str = "value";
const TYPE_OPTION_NAME: &str = "type";
const MESSAGE_OPTION_NAME: &str = "message";
const ACTION_TYPE_OPTION_NAME: &str = "action_type";
const ROLE_OPTION_NAME: &str = "role";
const DROP_TABLE_OPTION_NAME: &str = "drop_table";

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("create")
        .kind(CommandOptionType::SubCommand)
        .description("Create a new item.")
        .create_sub_option(|option| {
            option
                .name(NAME_OPTION_NAME)
                .description("The name of the item, cannot be blank.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|option| {
            option
                .name(DESCRIPTION_OPTION_NAME)
                .description("The description of the item.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(SELLABLE_OPTION_NAME)
                .description("Whether the item can be sold for the currency it corresponds to.")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(TRADEABLE_OPTION_NAME)
                .description("Whether the item can be traded between users.")
                .kind(CommandOptionType::Boolean)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(CURRENCY_OPTION_NAME)
                .description("The currency the item corresponds to.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(VALUE_OPTION_NAME)
                .description("The value of the item in terms of the currency it corresponds to.")
                .kind(CommandOptionType::Number)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(TYPE_OPTION_NAME)
                .description("How the item behaves.")
                .kind(CommandOptionType::String)
                .required(false)
                .add_string_choice("Trophy", "Trophy")
                .add_string_choice("Consumable", "Consumable")
                .add_string_choice("InstantConsumable", "InstantConsumable")
        })
        .create_sub_option(|option| {
            option
                .name(MESSAGE_OPTION_NAME)
                .description("The message to send when the item is used. Ignored when trophy.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(ACTION_TYPE_OPTION_NAME)
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
                .name(ROLE_OPTION_NAME)
                .description(
                    "The role to give when the item is used. Ignored when trophy or action_type is None or Lootbox."
                )
                .kind(CommandOptionType::Role)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(DROP_TABLE_OPTION_NAME)
                .description(
                    "The drop table to use when the item is used. Ignored when trophy or action_type is None or Role."
                )
                .kind(CommandOptionType::String)
                .required(false)
        });
    option
}
