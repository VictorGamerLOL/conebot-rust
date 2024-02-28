use anyhow::{ anyhow, Result };
use futures::future::try_join_all;
use serenity::{
    all::{ CommandInteraction, CommandOptionType, UserId },
    builder::{
        CreateCommand,
        CreateCommandOption,
        CreateEmbed,
        CreateEmbedAuthor,
        EditInteractionResponse,
    },
    http::{ CacheHttp, Http },
    model::{ prelude::{ GuildId, Member }, user::User, Colour },
};

use crate::{
    db::{ models::{ Balance, Balances, Currency }, uniques::DbGuildId, ArcTokioRwLockOption },
    event_handler::command_handler::CommandOptions,
    ACCENT_COLOUR,
};

#[allow(clippy::unused_async)]
pub async fn run<'a>(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl CacheHttp + AsRef<Http> + Clone
) -> Result<()> {
    let guild_id: DbGuildId = command.guild_id
        .ok_or_else(|| anyhow!("Command not allowed in DMs."))?
        .into();
    let opts = parse_options(options, guild_id, &http).await?;

    let possible_user = opts.user.as_ref();

    let (tmp_user, member) = if let Some(u) = possible_user {
        (&u.0, u.1.as_ref())
    } else {
        let user = &command.user;
        let member = command.member
            .as_ref()
            .ok_or_else(|| anyhow!("DMs not allowed"))?
            .as_ref();
        (user, member)
    };

    let user = (tmp_user, member);

    let balances = Balances::try_from_user(guild_id, user.0.id.into()).await?;

    let embed = if let Some(c) = opts.currency {
        single_currency(c, &balances, user, command).await?
    } else {
        multi_currency(balances, user, command).await?
    };

    command.edit_response(
        http,
        EditInteractionResponse::new().add_embed(embed).content("\u{200b}")
    ).await?;
    Ok(())
}

async fn multi_currency<'a>(
    balances: std::sync::Arc<tokio::sync::Mutex<Option<Balances>>>,
    user: (&User, &Member),
    command: &'a CommandInteraction
) -> Result<CreateEmbed, anyhow::Error> {
    let balances = balances.lock().await;
    let balances_ = balances
        .as_ref()
        .ok_or_else(|| {
            anyhow!("This user's balances are already being used in a breaking operation.")
        })?;
    let embed = multi_currency_embed(
        balances_.balances(),
        user.1,
        command.member.as_ref().ok_or_else(|| anyhow!("DMs not allowed"))?
    ).await?.colour(ACCENT_COLOUR);
    drop(balances);
    Ok(embed)
}

async fn single_currency<'a>(
    c: std::sync::Arc<tokio::sync::RwLock<Option<Currency>>>,
    balances: &'a std::sync::Arc<tokio::sync::Mutex<Option<Balances>>>,
    user: (&User, &Member),
    command: &CommandInteraction
) -> Result<CreateEmbed, anyhow::Error> {
    let currency = c.read().await;
    let currency_ = currency
        .as_ref()
        .ok_or_else(|| anyhow!("Currency is being used in a breaking operation."))?;
    let mut balances = balances.lock().await;
    let balances_ = balances
        .as_mut()
        .ok_or_else(|| {
            anyhow!("This user's balances are already being used in a breaking operation.")
        })?;
    let balance = balances_
        .balances()
        .iter()
        .find(|b| b.curr_name() == currency_.curr_name());
    let balance = if let Some(b) = balance {
        b
    } else {
        balances_.create_balance(currency_.curr_name().to_owned().into_string()).await?
    };
    let embed = single_currency_embed(
        balance,
        currency_,
        user.1,
        command.member.as_ref().ok_or_else(|| anyhow!("DMs not allowed"))?
    ).colour(ACCENT_COLOUR);
    drop(balances);
    drop(currency);
    Ok(embed)
}

fn single_currency_embed<'a>(
    balance: &'a Balance,
    currency: &'a Currency,
    target: &'a Member,
    executor: &'a Member
) -> CreateEmbed {
    let author = CreateEmbedAuthor::new(executor.display_name()).icon_url(executor.face());
    CreateEmbed::default()
        .title(
            format!(
                "{}'s balance for {}{}",
                target.display_name(),
                currency.symbol(),
                currency.curr_name().as_str()
            )
        )
        .description(format!("{}{}", currency.symbol(), balance.amount()))
        .colour(Colour::DARK_GREEN)
        .thumbnail(target.face())
        .timestamp(chrono::Utc::now())
        .author(author)
}

#[allow(clippy::unused_async)]
async fn multi_currency_embed(
    balances: &[Balance],
    target: &Member,
    executor: &Member
) -> Result<CreateEmbed> {
    let mut field_data: Vec<(String, String, bool)> = Vec::new();
    let t = try_join_all(
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
        let title = format!("{symbol}{n}");
        let description = format!("{symbol}{a}");
        field_data.push((title, description, true));
        drop(currency);
    }
    let author = CreateEmbedAuthor::new(executor.display_name()).icon_url(executor.face());
    Ok(
        CreateEmbed::default()
            .title(format!("{}'s balances", target.display_name()))
            .description(format!("{}'s balances for all currencies", target.display_name()))
            .colour(Colour::DARK_GREEN)
            .thumbnail(target.face())
            .fields(field_data)
            .timestamp(chrono::Utc::now())
            .author(author)
    )
}

struct Options {
    user: Option<(User, Box<Member>)>,
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
    let user: Option<UserId> = options.get_user_value("user").transpose()?;
    let currency: Option<String> = options.get_string_value("currency").transpose()?;

    let currency: Option<ArcTokioRwLockOption<Currency>> = if let Some(currency) = currency {
        Currency::try_from_name(guild_id, currency).await?
    } else {
        None
    };

    let user: Option<(User, Box<Member>)> = if let Some(u) = user {
        let guild_id: GuildId = guild_id.into();
        let member = guild_id.member(&http, u).await?;
        Some((u.to_user(http).await?, Box::new(member)))
    } else {
        None
    };
    Ok(Options { user, currency })
}

pub fn command() -> CreateCommand {
    CreateCommand::new("balance")
        .description("Check your balance or someone else's for one currency or all of them.")
        .dm_permission(false)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "user",
                "The user to check the balance of."
            ).required(false)
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "currency",
                "The currency to check the balance of."
            ).required(false)
        )
}
