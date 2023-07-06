use anyhow::{ anyhow, Result };
use serenity::builder::CreateApplicationCommandOption;
use serenity::http::Http;
use serenity::model::application::command::CommandOptionType;
use serenity::model::prelude::application_command::{
    ApplicationCommandInteraction,
    CommandDataOption,
    CommandDataOptionValue,
};
use serenity::model::user::User;

#[allow(clippy::unused_async)] // TODO
#[allow(unused)]
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync
) -> Result<()> {
    let mut name: String = String::new();
    let mut amount: f64 = 0.0;
    let mut user: User = User::default();
    for option in options {
        match option.name.as_str() {
            "name" => {
                name = option.value
                    .clone()
                    .ok_or(anyhow!("No currency name found."))?
                    .as_str()
                    .ok_or(anyhow!("Failed to convert currency name to str."))?
                    .to_owned();
            }
            "amount" => {
                amount = option.value
                    .clone()
                    .ok_or(anyhow!("No amount found."))?
                    .as_f64()
                    .ok_or(anyhow!("Failed to convert amount to f64."))?;
            }
            "user" => {
                user = match option.resolved.clone().ok_or(anyhow!("Failed to resolve user."))? {
                    CommandDataOptionValue::User(u, _) => u,
                    _ => {
                        return Err(anyhow!("Failed to resolve user."));
                    }
                };
            }
            _ => {
                return Err(anyhow!("Unknown option: {}", option.name));
            }
        }
    }
    Ok(())
}

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("give")
        .description("Give a user currency.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|o| {
            o.name("name")
                .description("The name of the currency to give.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name("amount")
                .description("The amount of currency to give.")
                .kind(CommandOptionType::Number)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name("user")
                .description("The user to give currency to.")
                .kind(CommandOptionType::User)
                .required(true)
        });
    todo!()
}
