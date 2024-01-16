use std::{ sync::Arc, time::Duration };

use anyhow::Result;
use chrono::Utc;
use serenity::{
    all::{ ActionRow, CommandInteraction, CommandOptionType, Message, ReactionType },
    builder::{
        CreateActionRow,
        CreateButton,
        CreateCommandOption,
        CreateEmbed,
        CreateInteractionResponse,
        EditInteractionResponse,
    },
    client::Context,
    gateway::{ ShardManager, ShardMessenger },
    http::{ CacheHttp, Http },
};
use tokio::{ join, try_join };

use crate::{
    db::models::{ drop_table::DropTablePart, DropTable },
    event_handler::command_handler::CommandOptions,
    util::paginator::Paginator,
};

use super::create;

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: &Context
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(||
        anyhow::anyhow!("Command cannot be done in DMs.")
    )?;
    let name = options
        .get_string_value("drop_table_name")
        .ok_or_else(|| anyhow::anyhow!("Name is required."))??;

    let drop_table = DropTable::try_from_name(
        guild_id.into(),
        std::borrow::Cow::from(&name),
        None
    ).await?;
    let drop_table = drop_table.read().await;
    let drop_table_ = if let Some(drop_table) = drop_table.as_ref() {
        if drop_table.drop_table_parts().is_empty() {
            drop_table.invalidate_cache_lenient().await;
            anyhow::bail!("Drop table does not exist.");
        }
        drop_table
    } else {
        anyhow::bail!("Drop table does not exist.");
    };

    let drop_table_parts = drop_table_.drop_table_parts().iter().collect::<Vec<&DropTablePart>>();
    let mut paginator = Paginator::new(drop_table_parts, 10)?;
    let (row, [button_first_id, button_previous_id, button_next_id, button_last_id]) =
        create_buttons();
    let mut embed = create_embed(paginator.current_page(), &name);

    let message = command.edit_response(
        &http,
        EditInteractionResponse::new().embed(embed).components(vec![row])
    ).await?;

    let mut paged_count = 0;
    loop {
        if paged_count >= 50 {
            // Hard limit of 50 scrolls of pages to prevent
            // the user from permanently locking a drop table
            // from being edited.
            break;
        }
        let Some(msg) = message
            .await_component_interactions(<Context as AsRef<ShardMessenger>>::as_ref(http))
            .author_id(command.user.id)
            .custom_ids(
                vec![
                    button_first_id.clone(),
                    button_previous_id.clone(), // i only want interactions from those buttons
                    button_next_id.clone(),
                    button_last_id.clone()
                ]
            )
            .timeout(Duration::from_secs(15)).await else {
            break; // if nothing received break
        };

        let msg_clone = msg.clone();
        let http_clone = http.clone();
        let handle = tokio::spawn(async move {
            msg_clone.create_response(http_clone, CreateInteractionResponse::Acknowledge).await
        });

        let mut embed = match msg.data.custom_id.as_str() {
            id if id == button_first_id => create_embed(paginator.first_page(), &name),
            id if id == button_previous_id => {
                if let Some(p) = paginator.prev_page() {
                    create_embed(p, &name)
                } else {
                    paged_count += 1;
                    continue;
                }
            }
            id if id == button_next_id => {
                if let Some(p) = paginator.next_page() {
                    create_embed(p, &name)
                } else {
                    paged_count += 1;
                    continue;
                }
            }
            id if id == button_last_id => create_embed(paginator.last_page(), &name),
            _ => {
                break;
            }
        };
        paged_count += 1;
    }

    drop(drop_table);
    Ok(())
}

fn create_embed(parts: &[&DropTablePart], drop_table_name: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::new().title(drop_table_name);
    for part in parts {
        embed = embed.field(
            part.drop().name(),
            format!(
                "kind: {},\nweight: {},\nmin: {},\nmax: {}",
                part.drop().kind_as_str(),
                part.weight(),
                part.min(),
                part.max().map_or("N/A".to_owned(), |max| max.to_string())
            ),
            true
        );
    }
    embed
}

fn create_buttons() -> (CreateActionRow, [String; 4]) {
    let mut current_time = Utc::now();
    let mut button_first_id = format!("btn_first_{}", current_time);
    let mut button_previous_id = format!("btn_previous_{}", current_time);
    let mut button_next_id = format!("btn_next_{}", current_time);
    let mut button_last_id = format!("btn_last_{}", current_time);
    let mut button_first = CreateButton::new(button_first_id.clone())
        .emoji(ReactionType::Unicode("⏮️".to_owned()))
        .style(serenity::all::ButtonStyle::Primary);
    let mut button_previous = CreateButton::new(button_previous_id.clone())
        .emoji(ReactionType::Unicode("◀️".to_owned()))
        .style(serenity::all::ButtonStyle::Primary);
    let mut button_next = CreateButton::new(button_next_id.clone())
        .emoji(ReactionType::Unicode("▶️".to_owned()))
        .style(serenity::all::ButtonStyle::Primary);
    let mut button_last = CreateButton::new(button_last_id.clone())
        .emoji(ReactionType::Unicode("⏭️".to_owned()))
        .style(serenity::all::ButtonStyle::Primary);

    let mut row = CreateActionRow::Buttons(
        vec![button_first, button_previous, button_next, button_last]
    );
    (row, [button_first_id, button_previous_id, button_next_id, button_last_id])
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "view_table",
        "View a drop table."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            "drop_table_name",
            "The name of the drop table."
        ).required(true)
    )
}
