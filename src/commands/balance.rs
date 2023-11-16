use anyhow::{ anyhow, Result };
use futures::future::try_join_all;
use serenity::{
    builder::{ CreateApplicationCommand, CreateEmbed, CreateEmbedAuthor },
    http::{ CacheHttp, Http },
    model::{
        prelude::{
            application_command::{
                ApplicationCommandInteraction,
                CommandDataOption,
                CommandDataOptionValue,
            },
            command::CommandOptionType,
            GuildId,
            Member,
            PartialMember,
        },
        user::User,
    },
    utils::Colour,
};

use crate::{
    db::{ id::DbGuildId, models::{ Balance, Balances, Currency }, ArcTokioRwLockOption },
    event_handler::command_handler::CommandOptions,
};

/// # Errors
/// TODO
///
/// # Panics
/// TODO
#[allow(clippy::unused_async)]
pub async fn run<'a>(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
    http: impl CacheHttp + AsRef<Http> + Clone
) -> Result<()> {
    let guild_id: DbGuildId = command.guild_id
        .ok_or_else(|| anyhow!("Command not allowed in DMs."))?
        .into();
    let opts = parse_options(options, guild_id, http.clone()).await?;

    let user = if let Some(u) = opts.user {
        u
    } else {
        let user = command.user.clone();
        let member = command.member.clone().ok_or_else(|| anyhow!("DMs not allowed"))?;
        (user, member)
    };

    let balances = Balances::try_from_user(guild_id, user.0.id.into()).await?;

    let embed = if let Some(c) = opts.currency {
        single_currency(c, &balances, guild_id.into(), &user, command).await?
    } else {
        multi_currency(balances, guild_id.into(), &user, command).await?
    };

    command.edit_original_interaction_response(http, |m|
        m.add_embed(embed).content("\u{200b}")
    ).await?;
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
        .ok_or_else(|| {
            anyhow!("This user's balances are already being used in a breaking operation.")
        })?;
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
    c: std::sync::Arc<tokio::sync::RwLock<Option<Currency>>>,
    balances: &'a std::sync::Arc<tokio::sync::Mutex<Option<Balances>>>,
    guild_id: GuildId,
    user: &'a (User, Member),
    command: &ApplicationCommandInteraction
) -> Result<CreateEmbed, anyhow::Error> {
    let mut currency = c.read().await;
    let currency_ = currency
        .as_ref()
        .ok_or_else(|| anyhow!("Currency is being used in a breaking operation."))?;
    let mut balances = balances.lock().await;
    let mut balances_ = balances
        .as_mut()
        .ok_or_else(|| {
            anyhow!("This user's balances are already being used in a breaking operation.")
        })?;
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
    embed.timestamp(chrono::Utc::now());
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
        balances
            .iter()
            .map(|b| async {
                b.currency().await.map(|c| anyhow::Ok((c, b.curr_name(), b.amount())))
            })
    ).await?
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|(c, _, _)| c.is_some())
        .map(|(a, b, c)| (a.unwrap(), b, c))
        .collect::<Vec<_>>();
    for (curr, n, a) in t {
        let currency = curr.read().await;
        let Some(currency_) = currency.as_ref() else {
            continue;
        };
        if !currency_.visible() {
            continue;
        }
        let symbol = currency_.symbol();
        let title = format!("{}{n}", symbol);
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
    currency: Option<ArcTokioRwLockOption<Currency>>,
}

async fn parse_options<'a>(
    options: CommandOptions,
    guild_id: DbGuildId,
    http: impl AsRef<Http> + CacheHttp + Clone + 'a
) -> Result<Options> {
    // Get the user, change it from Option<Result<(User, Option<PartialMember>)>> to Result<Option<(User, Option<PartialMember>)>>,
    // return if the result is Err, then map the Option<(User, Option<PartialMember>)> to Option<Result<(User, PartialMember)>>
    // then also change that to Result<Option<(User, PartialMember)>>, then return if the result is Err to finally get
    // Option<(User, PartialMember)>. Easy enough.
    let mut user: Option<(User, PartialMember)> = options
        .get_user_value("user")
        .transpose()?
        .map(
            |(u, m)| -> Result<(User, PartialMember)> {
                Ok((u, m.ok_or_else(|| anyhow!("DMs not allowed"))?))
            }
        )
        .transpose()?;
    let mut currency: Option<String> = options.get_string_value("currency").transpose()?;

    let currency: Option<ArcTokioRwLockOption<Currency>> = if let Some(currency) = currency {
        Currency::try_from_name(guild_id, currency).await?
    } else {
        None
    };

    let user: Option<(User, Member)> = if let Some((u, m)) = user {
        let guild_id: GuildId = guild_id.into();
        let member = guild_id.member(&http, u.id).await?;
        Some((u, member))
    } else {
        None
    };
    Ok(Options { user, currency })
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
