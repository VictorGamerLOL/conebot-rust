use drop_table::DropTable;
use serenity::{
    builder::{ CreateCommandOption, EditInteractionResponse },
    all::{ CommandOptionType, CommandInteraction },
    http::{ CacheHttp, Http },
};

use crate::{ event_handler::command_handler::CommandOptions, db::models::drop_table };

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + CacheHttp + Send + Sync
) -> anyhow::Result<()> {
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
    if let Some(drop_table) = drop_table.read().await.as_ref() {
        if drop_table.drop_table_parts().is_empty() {
            anyhow::bail!("Drop table does not exist.");
        }
    }
    DropTable::delete(drop_table, None).await?;

    command.edit_response(
        http,
        EditInteractionResponse::new().content("Drop table deleted.")
    ).await?;
    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "delete",
        "Delete a drop table."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            "drop_table_name",
            "The name of the drop table."
        ).required(true)
    )
}
