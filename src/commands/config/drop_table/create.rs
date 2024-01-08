use anyhow::{ anyhow, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::drop_table::{ builder::DropTableBuilder, DropTablePartOption },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?;
    let name = options.get_string_value("name").ok_or_else(|| anyhow!("Name is required."))??;
    let first_entry_name = options
        .get_string_value("first_entry_name")
        .ok_or_else(|| anyhow!("First entry name is required."))??;
    let first_entry_kind = options
        .get_string_value("first_entry_kind")
        .ok_or_else(|| anyhow!("First entry kind is required."))??;
    let first_entry_min = options
        .get_int_or_number_value("first_entry_min")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);
    let first_entry_max = options
        .get_int_or_number_value("first_entry_max")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);
    let first_entry_weight = options
        .get_int_or_number_value("first_entry_weight")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);

    let mut drop_table_builder = DropTableBuilder::new()
        .guild_id(Some(guild_id.into()))
        .drop_table_name(Some(name));

    let part_builder = drop_table_builder.new_part();

    match first_entry_kind.as_str() {
        "currency" => {
            part_builder.byref_drop(
                Some(DropTablePartOption::Currency {
                    currency_name: first_entry_name,
                })
            );
        }
        "item" => {
            part_builder.byref_drop(
                Some(DropTablePartOption::Item {
                    item_name: first_entry_name,
                })
            );
        }
        _ => {
            return Err(anyhow!("Invalid first entry kind."));
        }
    }

    part_builder
        .byref_min(first_entry_min)
        .byref_max(first_entry_max)
        .byref_weight(first_entry_weight);

    // -- No use of part_builder after this point --

    // let _: ArcTokioRwLockOption<DropTable> =
    drop_table_builder.build(None).await?;

    command.edit_response(
        http,
        EditInteractionResponse::new().content("Drop table created.")
    ).await?;

    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "create", "Create a drop table.")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "name",
                "The name of the drop table."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "first_entry_name",
                "The name of the first entry in the drop table."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "first_entry_kind",
                "The kind of the first entry in the drop table."
            )
                .required(true)
                .add_string_choice("currency", "currency")
                .add_string_choice("item", "item")
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "first_entry_min",
                "The minimum amount of the first entry to drop in the drop table. Must be positive. Default 1."
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "first_entry_max",
                "The maximum amount of the first entry to drop in the drop table. Must be positive. Default null."
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "first_entry_weight",
                "The weight of the first item to drop in the drop table. Must e positive Default 1."
            ).required(false)
        )
}
