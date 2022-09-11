mod bot;
mod discord;
mod minecraft;
use dotenvy::dotenv;
use log::info;
use std::env;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    env_logger::init();

    // make sure all environment variables exist at startup
    env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN env var not found");
    env::var("CONSOLE_CHANNEL_ID").expect("CONSOLE_CHANNEL_ID env var not found");
    env::var("SERVER_JAR_PATH").expect("SERVER_JAR_PATH env var not found");
    env::var("SERVER_MEMORY")
        .expect("SERVER_MEMORY env var not found")
        .parse::<u16>()
        .expect("SERVER_MEMORY env var has to be an u16 integer");
    let _ = env::var("MAX_PLAYERS").map(|max| {
        max.parse::<u8>()
            .expect("MAX_PLAYERS env var has to be an u8 integer")
    });

    info!("Starting up...");

    bot::init().await?;

    Ok(())
}
