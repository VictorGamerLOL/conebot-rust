use serenity::{
    builder::{ CreateApplicationCommandOption, CreateEmbed },
    model::prelude::{
        command::CommandOptionType,
        application_command::ApplicationCommandInteraction,
    },
    http::{ Http, CacheHttp },
};
use anyhow::{ Result, anyhow };
use tokio::sync::MutexGuard;
use crate::{
    event_handler::command_handler::CommandOptions,
    db::models::{ Currency, ToKVs },
    commands::currency,
};

#[derive(Debug)]
pub struct CurrencyConfigPrettifier<'a> {
    pub options: &'a Currency,
}

impl<'a> CurrencyConfigPrettifier<'a> {
    pub const fn new(options: &'a Currency) -> Self {
        Self {
            options,
        }
    }

    pub fn pretty(self) -> Result<CreateEmbed> {
        let mut embed = CreateEmbed::default();
        let kvs = self.options.try_to_kvs()?.into_iter();
        let mut channels_is_whitelist: bool = false;
        let mut roles_is_whitelist: bool = false;
        let mut embed_title: String = String::from("Config for {SYMBOL}{CURR_NAME}");
        // I need this only twice just in this function, so might as well just write it here.
        fn embed_field_default(k: &str, v: &str, embed: &mut CreateEmbed) {
            if k.is_empty() {
                return;
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
            embed.field(k_, v, true);
        }
        for (k, v) in kvs.as_ref() {
            match k.as_str() {
                // For fields that need special treatment like name symbol and stuff that need to be included in the title.
                "GuildId" => (),
                "CurrName" => {
                    embed_title = embed_title.replace("{CURR_NAME}", &v.replace('\"', ""));
                    embed_field_default(k, v, &mut embed);
                }
                "Symbol" => {
                    embed_title = embed_title.replace("{SYMBOL}", &v.replace('\"', ""));
                    embed_field_default(k, v, &mut embed);
                }
                "ChannelsIsWhitelist" => {
                    channels_is_whitelist = v.to_ascii_lowercase().parse()?;
                    embed_field_default(k, v, &mut embed);
                }
                "RolesIsWhitelist" => {
                    roles_is_whitelist = v.to_ascii_lowercase().parse()?;
                    embed_field_default(k, v, &mut embed);
                }
                "ChannelsWhitelist" | "ChannelsBlacklist" | "RolesWhitelist" | "RolesBlacklist" =>
                    (),
                &_ => {
                    embed_field_default(k, v, &mut embed);
                }
            }
        }
        for (k, v) in kvs {
            match k.as_str() {
                "GuildId" => (),
                "channelsWhitelist" => {
                    if channels_is_whitelist {
                        embed.field("Channels Whitelist", v, true);
                    }
                }
                "ChannelsBlacklist" => {
                    if !channels_is_whitelist {
                        embed.field("Channels Blacklist", v, true);
                    }
                }
                "RolesWhitelist" => {
                    if roles_is_whitelist {
                        embed.field("Roles Whitelist", v, true);
                    }
                }
                "RolesBlacklist" => {
                    if !roles_is_whitelist {
                        embed.field("Roles Blacklist", v, true);
                    }
                }
                &_ => {}
            }
        }
        embed.title(embed_title);
        Ok(embed)
    }
}

const COMMAND_OPTION_CURRENCY: &str = "currency";

pub async fn run(
    options: CommandOptions,
    command: &ApplicationCommandInteraction,
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

    command.edit_original_interaction_response(http, |m| { m.add_embed(embed) }).await?;

    Ok(())
}

#[must_use]
pub fn option() -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();
    option
        .name("list")
        .kind(CommandOptionType::SubCommand)
        .description("List out all of the config values for the specified currency.")
        .create_sub_option(|o| {
            o.name(COMMAND_OPTION_CURRENCY)
                .description("The currency to list the config values for.")
                .kind(CommandOptionType::String)
                .required(true)
        });
    option
}
