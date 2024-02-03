use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::item::{ self, fieldless::{ ItemActionTypeFieldless, ItemTypeFieldless } },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
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
        let type_ = ItemTypeFieldless::from_string(s)?;
        item_type_builder.type_(Some(type_));
    }
    if let Some(s) = message {
        item_type_builder.message(Some(s));
    }
    if let Some(s) = action_type {
        let action_type = ItemActionTypeFieldless::from_string(s)?;
        item_type_builder.action_type(Some(action_type));
    }
    item_type_builder.role(role.map(Into::into)).drop_table_name(drop_table);

    let item_type = item_type_builder.build()?;

    item_builder.item_type(Some(item_type));

    item_builder.build().await?;

    command.edit_response(http, EditInteractionResponse::new().content("Item created.")).await?;

    Ok(())
}

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

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "create", "Create a new item.")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                NAME_OPTION_NAME,
                "The name of the item, cannot be blank."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                DESCRIPTION_OPTION_NAME,
                "The description of the item."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                SELLABLE_OPTION_NAME,
                "Whether the item can be sold for the currency it corresponds to."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Boolean,
                TRADEABLE_OPTION_NAME,
                "Whether the item can be traded between users."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                CURRENCY_OPTION_NAME,
                "The currency the item corresponds to."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                VALUE_OPTION_NAME,
                "The value of the item in terms of the currency it corresponds to."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                TYPE_OPTION_NAME,
                "How the item behaves."
            )
                .add_string_choice("Trophy", "Trophy")
                .add_string_choice("Consumable", "Consumable")
                .add_string_choice("InstantConsumable", "InstantConsumable")
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                MESSAGE_OPTION_NAME,
                "The message to send when the item is used. Ignored when trophy."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                ACTION_TYPE_OPTION_NAME,
                "The type of action to perform when the item is used. Ignored when trophy."
            )
                .add_string_choice("None", "None")
                .add_string_choice("Role", "Role")
                .add_string_choice("Lootbox", "Lootbox")
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Role,
                ROLE_OPTION_NAME,
                "The role to give when the item is used. Ignored when trophy or action_type is None or Lootbox."
            )
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                DROP_TABLE_OPTION_NAME,
                "The drop table to use when the item is used. Ignored when trophy or action_type is None or Role."
            )
        )
}
