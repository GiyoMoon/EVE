use crate::discord::{handle_interaction, log_stdout, manage_status, set_commands, set_status};
use crate::minecraft::{ServerManager, ServerStatus};
use futures::StreamExt;
use log::{info, warn};
use std::fmt::Write;
use std::time::Duration;
use std::{env, sync::Arc};
use tokio::sync::RwLock;
use tokio::time;
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, Intents};
use twilight_http::Client;
use twilight_model::id::{marker::ChannelMarker, Id};

pub async fn init() -> Result<(), anyhow::Error> {
    let token = env::var("DISCORD_TOKEN").unwrap();
    let channel_id: Id<ChannelMarker> =
        Id::new(env::var("CONSOLE_CHANNEL_ID").unwrap().parse().unwrap());
    let max_players: u8 = env::var("MAX_PLAYERS").unwrap().parse().unwrap();

    let scheme = ShardScheme::Range {
        from: 0,
        to: 0,
        total: 1,
    };

    let (cluster, mut events) = Cluster::builder(token.clone(), Intents::empty())
        .shard_scheme(scheme)
        .build()
        .await?;

    let cluster = Arc::new(cluster);

    let cluster_c = cluster.clone();
    tokio::spawn(async move {
        cluster_c.up().await;
    });

    let http = Arc::new(Client::new(token));
    let (server, sender, mut receiver) = ServerManager::new();

    let application_id = http
        .current_user_application()
        .exec()
        .await?
        .model()
        .await?
        .id;

    tokio::spawn(set_commands(application_id, Arc::clone(&http)));

    let http_c = Arc::clone(&http);
    tokio::spawn(async move {
        set_status(Arc::clone(&cluster), ServerStatus::Offline).await;

        let cache = Arc::new(RwLock::new(String::new()));
        let timeout = Arc::new(RwLock::new(false));

        let mut current_status = ServerStatus::Offline;

        while let Some(msg) = receiver.recv().await {
            current_status =
                manage_status(Arc::clone(&cluster), current_status, max_players, &msg).await;

            let mut cache_w = cache.write().await;
            write!(cache_w, "\n{}", msg).unwrap();

            if !*timeout.read().await {
                let mut timeout_w = timeout.write().await;
                *timeout_w = true;

                let cached_c = Arc::clone(&cache);
                let timeout_c = Arc::clone(&timeout);
                let http_cc = Arc::clone(&http_c);
                tokio::spawn(async move {
                    // Timeout can't be lower than 800 ms due to Discord's rate limit
                    time::sleep(Duration::from_millis(800)).await;

                    let mut cache_w = cached_c.write().await;
                    let mut timeout_w = timeout_c.write().await;
                    let send_result =
                        log_stdout(Arc::clone(&http_cc), cache_w.to_string(), channel_id).await;
                    if let Err(e) = send_result {
                        warn!("Failed to send logs to Discord channel: {}", e);
                    }
                    *cache_w = String::new();
                    *timeout_w = false;
                });
            }
        }

        let cache = cache.read().await;
        if !cache.is_empty() {
            let send_result = log_stdout(http_c, cache.to_string(), channel_id).await;
            if let Err(e) = send_result {
                warn!("Failed to send logs to Discord channel: {}", e);
            }
        }
    });

    while let Some((_, event)) = events.next().await {
        match event {
            Event::InteractionCreate(interaction) => {
                match interaction.0.channel_id {
                    // check if interaction comes from configured channel
                    Some(c_id) if c_id == channel_id => {
                        handle_interaction(
                            application_id,
                            Arc::clone(&http),
                            Arc::clone(&server),
                            sender.clone(),
                            interaction,
                        )
                        .await?;
                    }
                    _ => {}
                }
            }
            Event::Ready(_) => {
                info!("Bot started!");
            }
            _ => {}
        };
    }

    Ok(())
}
