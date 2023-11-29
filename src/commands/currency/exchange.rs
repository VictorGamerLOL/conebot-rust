use anyhow::{ anyhow, bail, Result };
use serenity::{
    all::{ CommandInteraction, CommandOptionType },
    builder::{ CreateCommandOption, EditInteractionResponse },
    http::{ CacheHttp, Http },
};
use tokio::join;

use crate::{
    db::models::Currency,
    event_handler::command_handler::CommandOptions,
    mechanics::exchange::exchange,
};

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
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
        .ok_or_else(|| anyhow!("Command can't be performed in DMs"))?;

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

    command.edit_response(
        http,
        EditInteractionResponse::new().content(
            format!(
                "You gave {} {}{} and got {} {}{}.",
                amount,
                input.symbol(),
                input.curr_name().as_str(),
                given,
                output.symbol(),
                output.curr_name().as_str()
            )
        )
    ).await?;

    Ok(())
}

const INPUT_OPTION_NAME: &str = "input";
const OUTPUT_OPTION_NAME: &str = "output";
const AMOUNT_OPTION_NAME: &str = "amount";

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(CommandOptionType::SubCommand, "exchange", "Exchange currency.")
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                INPUT_OPTION_NAME,
                "The currency to exchange."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                OUTPUT_OPTION_NAME,
                "The currency to exchange to."
            ).required(true)
        )
        .add_sub_option(
            CreateCommandOption::new(
                CommandOptionType::Number,
                AMOUNT_OPTION_NAME,
                "The amount to exchange from input to output."
            ).required(true)
        )
}
