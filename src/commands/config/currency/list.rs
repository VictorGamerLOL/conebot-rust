use crate::{
    commands::currency,
    db::models::{ Currency, ToKVs },
    event_handler::command_handler::CommandOptions,
};
use anyhow::{ anyhow, Result };
use serenity::{
    builder::{ CreateCommandOption, CreateEmbed, EditInteractionResponse },
    http::{ CacheHttp, Http },
    all::{ CommandInteraction, CommandOptionType },
};
use tokio::sync::MutexGuard;

#[derive(Debug)]
pub struct CurrencyConfigPrettifier<'a> {
    pub options: &'a Currency,
}

impl<'a> CurrencyConfigPrettifier<'a> {
    pub const fn new(options: &'a Currency) -> Self {
        Self { options }
    }

    pub fn pretty(self) -> Result<CreateEmbed> {
        let mut embed = CreateEmbed::default();
        let kvs = self.options.try_to_kvs()?.into_iter();
        let mut channels_is_whitelist: bool = false;
        let mut roles_is_whitelist: bool = false;
        let mut embed_title: String = String::from("Config for {SYMBOL}{CURR_NAME}");
        // I need this only twice just in this function, so might as well just write it here.
        fn embed_field_default(k: &str, v: &str, mut embed: CreateEmbed) -> CreateEmbed {
            if k.is_empty() {
                return embed;
            }
            let mut k = k.chars();
            let mut k_ = String::new();
            k_.push(k.next().unwrap().to_ascii_uppercase());
            for c in k {
                if c.is_ascii_uppercase() {
                    k_.push(' ');
                }
                k_.push(c);
            }
            embed = embed.field(k_, v, true);
            embed
        }
        for (k, v) in kvs.as_ref() {
            match k.as_str() {
                // For fields that need special treatment like name symbol and stuff that need to be included in the title.
                "GuildId" => (),
                "CurrName" => {
                    embed_title = embed_title.replace("{CURR_NAME}", &v.replace('\"', ""));
                    embed = embed_field_default(k, v, embed);
                }
                "Symbol" => {
                    embed_title = embed_title.replace("{SYMBOL}", &v.replace('\"', ""));
                    embed = embed_field_default(k, v, embed);
                }
                "ChannelsIsWhitelist" => {
                    channels_is_whitelist = v.to_ascii_lowercase().parse()?;
                    embed = embed_field_default(k, v, embed);
                }
                "RolesIsWhitelist" => {
                    roles_is_whitelist = v.to_ascii_lowercase().parse()?;
                    embed = embed_field_default(k, v, embed);
                }
                | "ChannelsWhitelist"
                | "ChannelsBlacklist"
                | "RolesWhitelist"
                | "RolesBlacklist" => {}
                &_ => {
                    embed = embed_field_default(k, v, embed);
                }
            }
        }
        for (k, v) in kvs {
            match k.as_str() {
                "GuildId" => (),
                "channelsWhitelist" => {
                    if channels_is_whitelist {
                        embed = embed.field("Channels Whitelist", v, true);
                    }
                }
                "ChannelsBlacklist" => {
                    if !channels_is_whitelist {
                        embed = embed.field("Channels Blacklist", v, true);
                    }
                }
                "RolesWhitelist" => {
                    if roles_is_whitelist {
                        embed = embed.field("Roles Whitelist", v, true);
                    }
                }
                "RolesBlacklist" => {
                    if !roles_is_whitelist {
                        embed = embed.field("Roles Blacklist", v, true);
                    }
                }
                &_ => {}
            }
        }
        embed = embed.title(embed_title);
        Ok(embed)
    }
}

const COMMAND_OPTION_CURRENCY: &str = "currency";

pub async fn run(
    options: CommandOptions,
    command: &CommandInteraction,
    http: impl AsRef<Http> + Clone + CacheHttp
) -> Result<()> {
    let currency = options
        .get_string_value(COMMAND_OPTION_CURRENCY)
        .ok_or_else(|| anyhow!("Could not find currency."))??;

    let currency = Currency::try_from_name(
        command.guild_id.ok_or_else(|| anyhow!("Command may not be performed in DMs"))?.into(),
        currency.clone()
    ).await?.ok_or_else(move || anyhow!("Currency {} does not exist.", currency))?;

    let currency = currency.read().await;

    let currency_ = currency
        .as_ref()
        .ok_or_else(|| anyhow!("Currency is being used in breaking operation."))?;

    let embed = CurrencyConfigPrettifier::new(currency_).pretty()?;

    drop(currency);

    command.edit_response(http, EditInteractionResponse::new().embed(embed)).await?;

    Ok(())
}

pub fn option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::SubCommand,
        "list",
        "List out all of the config values for the specified currency."
    ).add_sub_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            COMMAND_OPTION_CURRENCY,
            "The currency to list the config values for."
        ).required(true)
    )
}
