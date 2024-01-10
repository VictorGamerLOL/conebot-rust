use anyhow::{ anyhow, Result, bail };
use serenity::{
    builder::{ CreateCommandOption, EditInteractionResponse },
    all::{ CommandOptionType, CommandInteraction },
    http::{ CacheHttp, Http },
};

use crate::{
    event_handler::command_handler::CommandOptions,
    db::models::{ DropTable, drop_table },
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Send + Sync
) -> Result<()> {
    let guild_id = command.guild_id.ok_or_else(||
        anyhow::anyhow!("Command cannot be done in DMs.")
    )?;
    let name = options
        .get_string_value("drop_table_name")
        .ok_or_else(|| anyhow::anyhow!("Name is required."))??;
    let entry_name = options
        .get_string_value("entry_name")
        .ok_or_else(|| anyhow::anyhow!("Entry name is required."))??;

    let drop_table = DropTable::try_from_name(
        guild_id.into(),
        std::borrow::Cow::from(&name),
        None
    ).await?;
    let mut drop_table = drop_table.write().await;
    let Some(drop_table_) = drop_table.as_mut() else {
        bail!("Drop table is being used in a breaking operation.")
    };
    drop_table_.delete_part(&entry_name, None).await?;
    drop(drop_table);
    command.edit_response(http, EditInteractionResponse::new().content("Entry deleted.")).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "delete_entry",
        "Delete an entry from a drop table."
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
                "The name of the entry to delete."
            ).required(true)
        )
}
