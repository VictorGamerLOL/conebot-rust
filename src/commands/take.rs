use serenity::{
    model::prelude::application_command::{ CommandDataOption, ApplicationCommandInteraction },
    http::{ CacheHttp, Http },
};

pub async fn run(
    options: &[CommandDataOption],
    command: &ApplicationCommandInteraction,
    http: impl AsRef<Http> + Clone + CacheHttp
) {
    todo!()
}
