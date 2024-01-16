use std::borrow::Cow;

use anyhow::{ anyhow, Result };
use serenity::{
    all::{ ActionRow, ButtonStyle, CommandInteraction, CommandOptionType, ReactionType },
    builder::{
        CreateActionRow,
        CreateButton,
        CreateCommandOption,
        CreateEmbed,
        CreateInteractionResponse,
        EditInteractionResponse,
    },
    client::Context,
    http::{ CacheHttp, Http },
};

use crate::{
    db::models::{ drop_table::{ builder::DropTablePartBuilder, DropTablePartOption }, DropTable },
    event_handler::command_handler::{ CommandOptions, IntOrNumber },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be done in DMs."))?;
    let name = options
        .get_string_value("drop_table_name")
        .ok_or_else(|| anyhow!("Name is required."))??;
    let first_entry_name = options
        .get_string_value("entry_name")
        .ok_or_else(|| anyhow!("First entry name is required."))??;
    let first_entry_kind = options
        .get_string_value("entry_kind")
        .ok_or_else(|| anyhow!("First entry kind is required."))??;
    let first_entry_min = options
        .get_int_or_number_value("min")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);
    let first_entry_max = options
        .get_int_or_number_value("max")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);
    let first_entry_weight = options
        .get_int_or_number_value("weight")
        .transpose()?
        .map(IntOrNumber::cast_to_i64);

    let mut drop_table = DropTable::try_from_name(guild_id.into(), Cow::from(&name), None).await?;
    let mut drop_table = drop_table.write().await;

    let mut drop_table_ = drop_table
        .as_mut()
        .ok_or_else(|| anyhow!("Drop table is being used in breaking operation."))?;

    if drop_table_.drop_table_parts().is_empty() {
        let yes_id = format!("yes_add_entry_{}", chrono::Utc::now().timestamp_millis());
        let no_id = format!("no_add_entry_{}", chrono::Utc::now().timestamp_millis());
        let msg = command.edit_response(&http, warn_prompt(&yes_id, &no_id)).await?;

        let inter = msg
            .await_component_interaction(http)
            .author_id(command.user.id)
            .custom_ids(vec![yes_id, no_id.clone()]).await
            .ok_or_else(|| anyhow!("No component interaction received."))?;

        inter.create_response(http, CreateInteractionResponse::Acknowledge).await?;

        if inter.data.custom_id == no_id {
            command.edit_response(
                http,
                EditInteractionResponse::new()
                    .content("Ok, cancelled.")
                    .embeds(vec![])
                    .components(vec![])
            ).await?;
            return Ok(());
        }
    }

    let mut part_builder = drop_table_.new_part_builder();

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
            anyhow::bail!("Unknown entry kind.");
        }
    }

    part_builder
        .byref_min(first_entry_min)
        .byref_max(first_entry_max)
        .byref_weight(first_entry_weight);

    drop_table_.add_part(part_builder, None).await?;

    drop(drop_table);

    command.edit_response(
        http,
        EditInteractionResponse::new()
            .content("Drop table entry added.")
            .embeds(vec![])
            .components(vec![])
    ).await?;

    Ok(())
}

fn warn_prompt(yes_id: impl Into<String>, no_id: impl Into<String>) -> EditInteractionResponse {
    let mut response = EditInteractionResponse::new();

    let mut embed = CreateEmbed::new()
        .title("⚠️ Warning ⚠️")
        .description(
            "A drop table of this name does not exist. This command will create a new drop table. Would you like to continue?"
        );
    let mut button_yes = CreateButton::new(yes_id)
        .style(ButtonStyle::Success)
        .emoji(ReactionType::Unicode("✅".to_string()));
    let mut button_no = CreateButton::new(no_id)
        .style(ButtonStyle::Danger)
        .emoji(ReactionType::Unicode("✖️".to_string()));

    let mut action_row = CreateActionRow::Buttons(vec![button_yes, button_no]);

    response.add_embed(embed).components(vec![action_row])
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "add_entry",
        "Add an entry to a drop table."
    )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "drop_table_name",
                "The name of the drop table."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "entry_name",
                "The name of the entry to add."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "entry_kind",
                "The kind of the entry to add."
            )
                .required(true)
                .add_string_choice("currency", "currency")
                .add_string_choice("item", "item")
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "min",
                "The minimum amount of the entry to add."
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "max",
                "The maximum amount of the entry to add."
            ).required(false)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "weight",
                "The weight of the entry to add."
            ).required(false)
        )
}
