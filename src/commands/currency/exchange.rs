use serenity::{
    http::{ CacheHttp, Http },
    model::prelude::{
        application_command::ApplicationCommandInteraction,
        command::CommandOptionType,
    },
    builder::CreateApplicationCommandOption,
};
use anyhow::{ Result, bail, anyhow };
use tokio::join;

use crate::{
    event_handler::command_handler::CommandOptions,
    db::models::Currency,
    mechanics::exchange::exchange,
};

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + CacheHttp
) -> Result<()> {
    let input = options
        .get_string_value(INPUT_OPTION_NAME)
        .transpose()?
        .ok_or_else(|| anyhow!("No input currency was found"))?;
    let output = options
        .get_string_value(OUTPUT_OPTION_NAME)
        .transpose()?
        .ok_or_else(|| anyhow!("No output currency was found"))?;
    let amount = options
        .get_int_or_number_value(AMOUNT_OPTION_NAME)
        .transpose()?
        .ok_or_else(|| anyhow!("No amount was found"))?;
    let member = command.member
        .as_ref()
        .ok_or_else(|| anyhow!("Command can't be performed in DMs"))?
        .clone();

    let amount = amount.cast_to_f64();

    let input = Currency::try_from_name(command.guild_id.unwrap().into(), input).await?.ok_or_else(
        || anyhow!("Input currency not found")
    )?;
    let output = Currency::try_from_name(
        command.guild_id.unwrap().into(),
        output
    ).await?.ok_or_else(|| anyhow!("Output currency not found"))?;

    let (input, output) = join!(input.read(), output.read()); // gotta get that sweet concurrency

    let input = input
        .as_ref()
        .ok_or_else(|| anyhow!("Input currency is being used in a breaking operation"))?;
    let output = output
        .as_ref()
        .ok_or_else(|| anyhow!("Output currency is being used in a breaking operation"))?;

    let given = exchange(input, output, amount, member).await?;

    command.edit_original_interaction_response(http, |r| {
        r.content(
            format!(
                "You gave {} {}{} and got {} {}{}.",
                amount,
                input.symbol(),
                input.curr_name(),
                given,
                output.symbol(),
                output.curr_name()
            )
        )
    }).await?;

    Ok(())
}

const INPUT_OPTION_NAME: &str = "input";
const OUTPUT_OPTION_NAME: &str = "output";
const AMOUNT_OPTION_NAME: &str = "amount";

pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("exchange")
        .description("Exchange currency.")
        .kind(CommandOptionType::SubCommand)
        .create_sub_option(|o| {
            o.name(INPUT_OPTION_NAME)
                .description("The currency to exchange.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(OUTPUT_OPTION_NAME)
                .description("The currency to exchange to.")
                .kind(CommandOptionType::String)
                .required(true)
        })
        .create_sub_option(|o| {
            o.name(AMOUNT_OPTION_NAME)
                .description("The amount to exchange from input to output.")
                .kind(CommandOptionType::Number)
                .required(true)
        });
    option
}
