use serenity::{
    builder::CreateApplicationCommandOption,
    model::application::command::CommandOptionType,
    model::prelude::Mention,
};

use crate::db::models::{ Item, item::ItemTypeUpdateType };

pub async fn run(
    options: crate::event_handler::command_handler::CommandOptions,
    command: &serenity::model::application::interaction::application_command::ApplicationCommandInteraction,
    http: impl AsRef<serenity::http::Http> + serenity::http::CacheHttp + Clone + Send + Sync
) -> anyhow::Result<()> {
    let guild_id = command.guild_id.ok_or_else(||
        anyhow::anyhow!("Command may not be performed in DMs")
    )?;
    let item_name = options
        .get_string_value(NAME_OPTION_NAME)
        .ok_or_else(|| anyhow::anyhow!("Item name not found."))??;
    let mut field_name = options
        .get_string_value(FIELD_OPTION_NAME)
        .ok_or_else(|| anyhow::anyhow!("Field name not found."))??;
    let value = options
        .get_string_value(VALUE_OPTION_NAME)
        .ok_or_else(|| anyhow::anyhow!("Value not found."))??;
    field_name.make_ascii_lowercase();
    field_name = field_name.replace([' ', '-'], "_");

    let mut item = Item::try_from_name(guild_id.into(), item_name.clone()).await?;

    let mut item_ = item.write().await;

    let mut item__ = item_
        .as_mut()
        .ok_or_else(|| {
            anyhow::anyhow!("Item {} is being used in breaking operation", item_name)
        })?;

    let mut possible_fut = None;

    match field_name.as_str() {
        "name" => {
            possible_fut = Some(Item::update_name(item.clone(), value.clone()));
        }
        "description" | "desc" => item__.update_description(value, None).await?,
        "sellable" | "sell" => item__.update_sellable(value.parse()?, None).await?,
        "tradeable" | "trade" => item__.update_tradeable(value.parse()?, None).await?,
        "currency" | "currency_value" => item__.update_currency_value(value, None).await?,
        "value" => item__.update_value(value.parse()?, None).await?,
        "type" =>
            item__.update_item_type(
                item__
                    .item_type()
                    .update_auto(ItemTypeUpdateType::Type(value.to_ascii_lowercase().parse()?))?,
                None
            ).await?,
        "message" =>
            item__.update_item_type(
                item__.item_type().update_auto(ItemTypeUpdateType::Message(value))?,
                None
            ).await?,
        "action_type" | "action" =>
            item__.update_item_type(
                item__
                    .item_type()
                    .update_auto(
                        ItemTypeUpdateType::ActionType(value.to_ascii_lowercase().parse()?)
                    )?,
                None
            ).await?,
        "role" | "roleid" =>
            item__.update_item_type(
                item__.item_type().update_auto(
                    ItemTypeUpdateType::RoleId({
                        if value.contains('<') {
                            let mut mention: Mention = value.parse()?;
                            if let Mention::Role(role_id_) = mention {
                                role_id_.into()
                            } else {
                                anyhow::bail!("Invalid role mention.")
                            }
                        } else {
                            let num: u64 = value.parse()?;
                            num.into()
                        }
                    })
                )?,
                None
            ).await?,
        "drop_table" | "drop_table_name" =>
            item__.update_item_type(
                item__.item_type().update_auto(ItemTypeUpdateType::DropTableName(value))?,
                None
            ).await?,
        _ => anyhow::bail!("Field {} does not exist.", field_name),
    }

    drop(item_);
    if let Some(fut) = possible_fut {
        fut.await?;
    }

    command.edit_original_interaction_response(&http, |m| {
        m.content(format!("Edited item {}.", item_name))
    }).await?;
    Ok(())
}

pub const NAME_OPTION_NAME: &str = "name";
pub const FIELD_OPTION_NAME: &str = "field";
pub const VALUE_OPTION_NAME: &str = "value";

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("edit")
        .description("Edit an item.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|option| {
            option
                .name(NAME_OPTION_NAME)
                .description("The name of the item to edit.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|option| {
            option
                .name(FIELD_OPTION_NAME)
                .description("The field of the item to edit.")
                .kind(CommandOptionType::String)
                .required(false)
        })
        .create_sub_option(|option| {
            option
                .name(VALUE_OPTION_NAME)
                .description("The value to set the field to.")
                .kind(CommandOptionType::String)
                .required(false)
        });
    option
}
