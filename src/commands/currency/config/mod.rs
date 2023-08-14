pub mod list;

use serenity::{
    builder::CreateApplicationCommandOption,
    model::prelude::command::CommandOptionType,
};

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("config")
        .kind(CommandOptionType::SubCommandGroup)
        .description("Configure various things about currencies or view them.")
        .add_sub_option(list::option());
    option
}
