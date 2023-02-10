use crate::discord::{handle_interaction, log_stdout, manage_status, set_commands, set_status};
use crate::minecraft::{ServerManager, ServerStatus};
use log::{info, warn};
use std::fmt::Write;
use std::time::Duration;
use std::{env, sync::Arc};
use tokio::sync::mpsc::Receiver;
use tokio::sync::RwLock;
use tokio::time;
use twilight_gateway::{Event, Intents, MessageSender};
use twilight_gateway::{Shard, ShardId};
use twilight_http::Client;
use twilight_model::id::{marker::ChannelMarker, Id};

pub async fn init() -> Result<(), anyhow::Error> {
    let token = env::var("DISCORD_TOKEN").unwrap();

    let mut shard = Shard::new(ShardId::ONE, token.clone(), Intents::empty());
    let message_sender = shard.sender();

    let client = Arc::new(Client::new(token));
    let (server, sender, receiver) = ServerManager::new();

    let application_id = client.current_user_application().await?.model().await?.id;

    tokio::spawn(set_commands(application_id, client.clone()));

    let status = Arc::new(RwLock::new(ServerStatus::Offline));

    message_receiver(
        receiver,
        message_sender.clone(),
        status.clone(),
        client.clone(),
    );

    loop {
        match shard.next_event().await {
            Ok(event) => match event {
                Event::InteractionCreate(interaction) => {
                    handle_interaction(
                        application_id,
                        client.clone(),
                        server.clone(),
                        sender.clone(),
                        interaction,
                    )
                    .await?;
                }
                Event::Ready(_) => {
                    info!("Bot started!");
                    set_status(&message_sender, *status.read().await).await;
                }
                _ => {}
            },
            Err(source) => {
                warn!("Error receiving discord event: {source}");

                if source.is_fatal() {
                    break;
                }

                continue;
            }
        };
    }

    Ok(())
}

fn message_receiver(
    mut receiver: Receiver<String>,
    message_sender: MessageSender,
    status: Arc<RwLock<ServerStatus>>,
    client: Arc<Client>,
) {
    let channel_id: Id<ChannelMarker> =
        Id::new(env::var("CONSOLE_CHANNEL_ID").unwrap().parse().unwrap());
    let max_players: Option<u8> = env::var("MAX_PLAYERS").ok().map(|max| max.parse().unwrap());

    tokio::spawn(async move {
        let cache = Arc::new(RwLock::new(String::new()));
        let timeout = Arc::new(RwLock::new(false));

        while let Some(msg) = receiver.recv().await {
            let old_status = *status.read().await;
            let new_status = manage_status(&message_sender, old_status, max_players, &msg).await;

            if new_status != old_status {
                let mut status = status.write().await;
                *status = new_status;
            }

            let mut cache_w = cache.write().await;
            write!(cache_w, "\n{msg}").unwrap();

            if !*timeout.read().await {
                let mut timeout_w = timeout.write().await;
                *timeout_w = true;

                send_logs(channel_id, cache.clone(), timeout.clone(), client.clone());
            }
        }

        let cache = cache.read().await;
        if !cache.is_empty() {
            let send_result = log_stdout(client, cache.to_string(), channel_id).await;
            if let Err(e) = send_result {
                warn!("Failed to send logs to Discord channel: {e}");
            }
        }
    });
}

fn send_logs(
    channel_id: Id<ChannelMarker>,
    cached: Arc<RwLock<String>>,
    timeout: Arc<RwLock<bool>>,
    client: Arc<Client>,
) {
    tokio::spawn(async move {
        // Timeout can't be lower than 800 ms due to Discord's rate limit
        time::sleep(Duration::from_millis(800)).await;

        let mut cache_w = cached.write().await;
        let mut timeout_w = timeout.write().await;
        let send_result = log_stdout(client.clone(), cache_w.to_string(), channel_id).await;
        if let Err(e) = send_result {
            warn!("Failed to send logs to Discord channel: {e}");
        }
        *cache_w = String::new();
        *timeout_w = false;
    });
}
