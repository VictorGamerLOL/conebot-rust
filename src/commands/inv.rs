use std::time::Duration;

use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ ButtonStyle, CommandInteraction, ReactionType },
    builder::{
        CreateActionRow,
        CreateButton,
        CreateCommand,
        CreateEmbed,
        CreateEmbedAuthor,
        EditInteractionResponse,
    },
    client::Context,
};

use crate::{
    db::models::Inventory,
    event_handler::command_handler::CommandOptions,
    util::paginator::Paginator,
    ACCENT_COLOUR,
};

#[allow(clippy::option_if_let_else)]
/// Run the command.
///
/// # Errors
///
/// This function can return an error if there is a problem executing the command.
pub async fn run(_: CommandOptions, command: &CommandInteraction, ctx: &Context) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(|| anyhow!("Command cannot be used in DMs"))?;
    let user_id = command.user.id;
    let username = command.user.name.as_str();
    let icon = command.user.face();

    let user_inv = Inventory::try_from_user(guild_id.into(), user_id.into()).await?;
    let user_inv = user_inv.lock().await;

    let user_inv_ = user_inv
        .as_ref()
        .ok_or_else(|| anyhow!("User's inventory is being used in a breaking operation"))?;

    let entries = user_inv_
        .inventory()
        .iter()
        .map(|entry| (entry.item_name().to_owned(), entry.amount()))
        .collect::<Vec<_>>();

    drop(user_inv);

    let mut paginator = Paginator::new(entries, 10)?;

    let controls = inv_controls();
    let (first_button_id, next_button_id, prev_button_id, last_button_id) = (
        controls.first_button_id,
        controls.next_button_id,
        controls.prev_button_id,
        controls.last_button_id,
    );

    command.edit_response(
        &ctx,
        EditInteractionResponse::new()
            .embed(make_embed(paginator.first_page(), username, &icon))
            .components(vec![controls.row.clone()])
    ).await?;

    let response = command.get_response(&ctx).await?;

    loop {
        let interaction = response
            .await_component_interaction(ctx)
            .author_id(user_id)
            .custom_ids(
                vec![
                    first_button_id.clone(),
                    next_button_id.clone(),
                    prev_button_id.clone(),
                    last_button_id.clone()
                ]
            )
            .timeout(Duration::from_secs(30)).await;
        if let Some(i) = interaction {
            i.defer_ephemeral(&ctx).await?;
            let id: &str = &i.data.custom_id;
            let page = match id {
                id if first_button_id == id => paginator.first_page(),
                id if next_button_id == id => {
                    let pg = paginator.next_page();
                    if let Some(p) = pg {
                        p
                    } else {
                        paginator.current_page()
                    }
                }
                id if prev_button_id == id => {
                    let pg = paginator.prev_page();
                    if let Some(p) = pg {
                        p
                    } else {
                        paginator.current_page()
                    }
                }
                id if last_button_id == id => paginator.last_page(),
                _ => { bail!("Invalid button id") }
            };
            command.edit_response(
                &ctx,
                EditInteractionResponse::new().embed(make_embed(page, username, &icon))
            ).await?;
            i.delete_response(&ctx).await?;
        } else {
            break;
        }
    }

    Ok(())
}

fn make_embed(data: &[(String, i64)], username: &str, icon: &str) -> CreateEmbed {
    let author = CreateEmbedAuthor::new(username).icon_url(icon);
    let embed = CreateEmbed::default().title("Inventory").author(author);
    let mut description = String::new();
    for (item, amount) in data {
        description.push_str(&format!("**{item}** *x{amount}*\n"));
    }
    embed.description(description).colour(ACCENT_COLOUR)
}

struct InvControls {
    row: CreateActionRow,
    first_button_id: String,
    next_button_id: String,
    prev_button_id: String,
    last_button_id: String,
}

fn inv_controls() -> InvControls {
    let now = chrono::Utc::now();
    let first_id = format!("{now}first_page");
    let next_id = format!("{now}next_page");
    let prev_id = format!("{now}prev_page");
    let last_id = format!("{now}last_page");
    let first_button = CreateButton::new(first_id.clone())
        .emoji(ReactionType::Unicode("⏮️".to_owned()))
        .style(ButtonStyle::Primary);
    let last_button = CreateButton::new(last_id.clone())
        .emoji(ReactionType::Unicode("⏭️".to_owned()))
        .style(ButtonStyle::Primary);
    let next_button = CreateButton::new(next_id.clone())
        .emoji(ReactionType::Unicode("⏩".to_owned()))
        .style(ButtonStyle::Primary);
    let prev_button = CreateButton::new(prev_id.clone())
        .emoji(ReactionType::Unicode("⏪".to_owned()))
        .style(ButtonStyle::Primary);
    let action_row = CreateActionRow::Buttons(
        vec![first_button, prev_button, next_button, last_button]
    );
    InvControls {
        row: action_row,
        first_button_id: first_id,
        next_button_id: next_id,
        prev_button_id: prev_id,
        last_button_id: last_id,
    }
}
pub fn command() -> CreateCommand {
    CreateCommand::new("inv").description("View your inventory").dm_permission(false)
}
