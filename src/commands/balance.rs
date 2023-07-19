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
use futures::{ Stream, StreamExt, TryStream, TryStreamExt, future::{ join_all, try_join_all } };

use crate::db::{ models::{ Balances, Currency, Balance }, id::DbGuildId, ArcTokioMutexOption };

/// # Errors
/// TODO
///
/// # Panics
/// TODO
#[allow(clippy::unused_async)]
pub async fn run<'a>(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl CacheHttp + AsRef<Http> + Clone
) -> Result<()> {
    let guild_id: DbGuildId = command.guild_id
        .ok_or_else(|| anyhow!("Command not allowed in DMs."))?
        .into();
    let opts = parse_options(options, guild_id.clone(), http.clone()).await?;

    let user = if let Some(u) = opts.user {
        u
    } else {
        let user = command.user.clone();
        let member = command.member.clone().ok_or_else(|| anyhow!("DMs not allowed"))?;
        (user, member)
    };

    let balances = Balances::try_from_user(guild_id.clone(), user.0.id.into()).await?;

    let embed = if let Some(c) = opts.currency {
        single_currency(c, &balances, guild_id.try_into()?, &user, command).await?
    } else {
        multi_currency(balances, guild_id.try_into()?, &user, command).await?
    };

    command.edit_original_interaction_response(http, |m| {
        m.add_embed(embed).content("\u{200b}")
    }).await?;
    Ok(())
}

async fn multi_currency<'a>(
    balances: std::sync::Arc<tokio::sync::Mutex<Option<Balances>>>,
    guild_id: GuildId,
    user: &'a (User, Member),
    command: &'a ApplicationCommandInteraction
) -> Result<CreateEmbed, anyhow::Error> {
    let mut balances = balances.lock().await;
    let mut balances_ = balances
        .as_ref()
        .ok_or_else(||
            anyhow!("This user's balances are already being used in a breaking operation.")
        )?;
    let embed = multi_currency_embed(
        balances_.balances(),
        guild_id,
        &user.1.clone(),
        &command.member.clone().ok_or_else(|| anyhow!("DMs not allowed"))?
    ).await?;
    drop(balances);
    Ok(embed)
}

async fn single_currency<'a>(
    c: std::sync::Arc<tokio::sync::Mutex<Option<Currency>>>,
    balances: &'a std::sync::Arc<tokio::sync::Mutex<Option<Balances>>>,
    guild_id: GuildId,
    user: &'a (User, Member),
    command: &ApplicationCommandInteraction
) -> Result<CreateEmbed, anyhow::Error> {
    let mut currency = c.lock().await;
    let currency_ = currency
        .as_ref()
        .ok_or_else(|| anyhow!("Currency is being used in a breaking operation."))?;
    let mut balances = balances.lock().await;
    let mut balances_ = balances
        .as_mut()
        .ok_or_else(||
            anyhow!("This user's balances are already being used in a breaking operation.")
        )?;
    let mut balance = balances_
        .balances()
        .iter()
        .find(|b| b.curr_name() == currency_.curr_name());
    let balance = if let Some(b) = balance {
        b
    } else {
        balances_.create_balance(currency_.curr_name().to_owned()).await?
    };
    let embed = single_currency_embed(
        balance,
        currency_,
        guild_id,
        &user.1,
        &command.member.clone().ok_or_else(|| anyhow!("DMs not allowed"))?
    );
    drop(balances);
    drop(currency);
    Ok(embed)
}

fn single_currency_embed<'a>(
    balance: &'a Balance,
    currency: &'a Currency,
    guild: GuildId,
    target: &'a Member,
    executor: &'a Member
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

#[allow(clippy::unused_async)]
async fn multi_currency_embed(
    balances: &[Balance],
    guild: GuildId,
    target: &Member,
    executor: &Member
) -> Result<CreateEmbed> {
    let mut field_data: Vec<(String, String, bool)> = Vec::new();
    let mut t = try_join_all(
        balances.iter().map(|b| {
            let b2: &'static Balance = unsafe { std::mem::transmute(b) }; // ik what im doing ok?
            tokio::spawn(async move {
                let cur = b2.currency().await?;
                anyhow::Ok((cur, b2.curr_name.clone(), b2.amount))
            })
        })
    ).await?
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(c, _, _)| c.is_some())
        .map(|(a, b, c)| (a.unwrap(), b, c))
        .collect::<Vec<_>>();
    for (curr, n, a) in t {
        let currency = curr.lock().await;
        let Some(currency_) = currency.as_ref() else {
            continue;
        };
        let symbol = currency_.symbol().to_owned();
        let title = format!("{}{n}", symbol.clone());
        let description = format!("{symbol}{a}");
        field_data.push((title, description, true));
        drop(currency);
    }
    let mut embed = CreateEmbed::default();
    let mut author = CreateEmbedAuthor::default();
    author.name(executor.display_name()).icon_url(executor.face());
    embed.title(format!("{}'s balances", target.display_name()));
    embed.description(format!("{}'s balances for all currencies", target.display_name()));
    embed.colour(Colour::DARK_GREEN);
    embed.image(target.face());
    embed.fields(field_data);
    Ok(embed)
}

struct Options {
    user: Option<(User, Member)>,
    currency: Option<ArcTokioMutexOption<Currency>>,
}

async fn parse_options<'a>(
    options: &'a [CommandDataOption],
    guild_id: DbGuildId,
    http: impl AsRef<Http> + CacheHttp + Clone + 'a
) -> Result<Options> {
    let mut user: Option<(User, PartialMember)> = None;
    let mut currency: Option<String> = None;

    let options: Vec<(String, CommandDataOptionValue)> = options
        .iter()
        .cloned()
        .map(
            |o| -> Result<(String, CommandDataOptionValue)> {
                let res = o.resolved.ok_or_else(|| anyhow!("Failed to resolve {}", o.name))?;
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
                Some((u, m.ok_or_else(|| anyhow!("Did not find a member for user."))?))
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
        let guild_id: GuildId = guild_id.try_into()?;
        let member = guild_id.member(http.clone(), u.id).await?;
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
