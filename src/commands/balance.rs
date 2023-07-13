use anyhow::{ anyhow, Result };
use serenity::{
    builder::{ CreateApplicationCommand, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter },
    http::{ Http, CacheHttp },
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction,
                CommandDataOption,
                CommandDataOptionValue,
            },
            Member,
            command::CommandOptionType,
            GuildId,
            PartialMember,
            Embed,
        },
        user::User,
        Timestamp,
    },
    utils::Colour,
};
use time::OffsetDateTime;
use futures::{ Stream, StreamExt, TryStream, TryStreamExt };

use crate::db::{ models::{ Balances, Currency, Balance }, id::DbGuildId, ArcTokioMutexOption };

/// # Errors
/// TODO
#[allow(clippy::unused_async)]
pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Send + Sync + Clone + CacheHttp
) -> Result<()> {
    let guild_id = command.guild_id.ok_or(anyhow!("Cannot perform command in DM."))?;
    let mut opts = parse_options(options, guild_id.into(), http.clone()).await?;

    let user = if let Some(u) = opts.user {
        u
    } else {
        (
            command.user.clone(),
            command.member.clone().ok_or(anyhow!("Cannot find member for user."))?,
        )
    };

    let balances = Balances::try_from_user(guild_id.into(), user.0.id.into()).await?;

    let embed = if let Some(c) = opts.currency {
        let mut currency = c.lock().await;
        let currency_ = currency
            .as_ref()
            .ok_or(anyhow!("Currency is being used in a breaking operation."))?;
        let mut balances = balances.lock().await;
        let mut balances_ = balances
            .as_mut()
            .ok_or(
                anyhow!("This user's balances are already being used in a breaking operation.")
            )?;
        let mut balance = balances_
            .balances()
            .iter()
            .find(|b| b.curr_name() == currency_.curr_name());
        let balance = if let Some(b) = balance {
            b
        } else {
            balances_.create_balance(currency_.curr_name().to_owned()).await?;
            balances_
                .balances()
                .iter()
                .find(|b| b.curr_name() == currency_.curr_name())
                .ok_or(
                    anyhow!(
                        "Created balance for specified user but could not find it afterwards, strange."
                    )
                )?
        };
        single_currency_embed(
            balance,
            currency_,
            guild_id,
            &user.1,
            &command.member.clone().ok_or(anyhow!("DMs not allowed"))?
        )
    } else {
        let mut balances = balances.lock().await;
        let mut balances_ = balances
            .as_ref()
            .ok_or(
                anyhow!("This user's balances are already being used in a breaking operation.")
            )?;
        multi_currency_embed(
            balances_.balances(),
            guild_id,
            &user.1.clone(),
            &command.member.clone().ok_or(anyhow!("DMs not allowed"))?
        ).await?
    };

    command.edit_original_interaction_response(http, |m| {
        m.add_embed(embed).content("\u{200b}") /* Zero-width space */
    }).await?;
    Ok(())
}

fn single_currency_embed(
    balance: &Balance,
    currency: &Currency,
    guild: GuildId,
    target: &Member,
    executor: &Member
) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    let mut author = CreateEmbedAuthor::default();
    author.name(executor.display_name()).icon_url(executor.face());
    embed.title(
        format!(
            "{}'s balance for {}{}",
            target.display_name(),
            currency.symbol(),
            currency.curr_name()
        )
    );
    embed.description(format!("{}{}", currency.symbol(), balance.amount()));
    embed.colour(Colour::DARK_GREEN);
    embed.image(target.face());
    embed.timestamp(OffsetDateTime::now_utc());
    embed
}

async fn multi_currency_embed(
    balances: &[Balance],
    guild: GuildId,
    target: &Member,
    executor: &Member
) -> Result<CreateEmbed> {
    let mut embed = CreateEmbed::default();
    let mut author = CreateEmbedAuthor::default();
    author.name(executor.display_name()).icon_url(executor.face());
    embed.title(format!("{}'s balances", target.display_name()));
    embed.description("Balances for all currencies");
    let mut count = balances.len();
    let mut fields = futures::stream::iter(balances.iter());
    let mut fields = fields
        .then(|b| async {
            let mut currency = b.currency().await?.ok_or(anyhow!("What the fuck #1."))?;
            let mut currency = currency.lock().await;
            if currency.is_none() {
                return anyhow::Ok(("Error".to_owned(), "Error".to_owned(), true));
            }
            let currency_ = currency.as_ref().ok_or(anyhow!("What the fuck #2."))?;
            let field: (String, String, bool) = (
                format!("{}{}", currency_.symbol(), currency_.curr_name()),
                format!("{}{}", currency_.symbol(), b.amount()),
                true,
            );
            drop(currency);
            anyhow::Ok(field)
        })
        .map(|i| async { i })
        .buffered(count)
        .try_collect::<Vec<_>>().await?;
    embed.fields(fields);
    embed.colour(Colour::DARK_GREEN);
    embed.image(target.face());
    Ok(embed)
}

struct Options {
    user: Option<(User, Member)>,
    currency: Option<ArcTokioMutexOption<Currency>>,
}

async fn parse_options(
    options: &[CommandDataOption],
    guild_id: DbGuildId,
    http: impl AsRef<Http> + CacheHttp + Clone + Send + Sync
) -> Result<Options> {
    let mut user: Option<(User, PartialMember)> = None;
    let mut currency: Option<String> = None;

    let options: Vec<(String, CommandDataOptionValue)> = options
        .iter()
        .cloned()
        .map(
            |o| -> Result<(String, CommandDataOptionValue)> {
                let res = o.resolved.ok_or(anyhow!("Failed to resolve {}", o.name))?;
                Ok((o.name, res))
            }
        )
        .collect::<Result<Vec<(String, CommandDataOptionValue)>>>()?;

    for (name, option) in options {
        if name == *"currency" {
            currency = if let CommandDataOptionValue::String(s) = option {
                Some(s)
            } else {
                return Err(anyhow!("Did not find a string for currency name."));
            };
        } else if name == *"user" {
            user = if let CommandDataOptionValue::User(u, m) = option {
                Some((u, m.ok_or(anyhow!("Did not find a member for user."))?))
            } else {
                return Err(anyhow!("Did not find a user for user."));
            };
        }
    }

    let currency: Option<ArcTokioMutexOption<Currency>> = if let Some(currency) = currency {
        Currency::try_from_name(guild_id.clone(), currency).await?
    } else {
        None
    };

    let user: Option<(User, Member)> = if let Some((u, m)) = user {
        let guild_id: GuildId = guild_id.into();
        let member = guild_id.member(http, &u.id).await?;
        Some((u, member))
    } else {
        None
    };
    Ok(Options {
        user,
        currency,
    })
}

#[must_use]
pub fn application_command() -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();
    command
        .name("balance")
        .description("Check your balance or someone else's for one currency or one of them.")
        .dm_permission(false)
        .create_option(|o| {
            o.name("user")
                .description("The user to check the balance of.")
                .kind(CommandOptionType::User)
                .required(false)
        })
        .create_option(|o| {
            o.name("currency")
                .description("The currency to check the balance of.")
                .kind(CommandOptionType::String)
                .required(false)
        });
    command
}
