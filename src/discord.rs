use crate::minecraft::{ServerCommand, ServerConfig, ServerManager, ServerStatus};
use log::{info, warn};
use std::{env, sync::Arc};
use tokio::sync::mpsc;
use twilight_gateway::MessageSender;
use twilight_http::{client::InteractionClient, Client};
use twilight_model::{
    application::{
        command::CommandType,
        interaction::{application_command::CommandOptionValue, InteractionData},
    },
    gateway::{
        payload::{incoming::InteractionCreate, outgoing::UpdatePresence},
        presence::{Activity, ActivityType, MinimalActivity, Status},
    },
    http::{
        attachment::Attachment,
        interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    },
    id::{
        marker::{ApplicationMarker, ChannelMarker, InteractionMarker},
        Id,
    },
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

pub(crate) async fn log_stdout(
    client: Arc<Client>,
    content: String,
    channel_id: Id<ChannelMarker>,
) -> anyhow::Result<()> {
    if content.chars().count() <= 2000 {
        client.create_message(channel_id).content(&content)?.await?;
    } else {
        let attachment = Attachment::from_bytes("console.log".to_string(), content.into_bytes(), 1);
        client
            .create_message(channel_id)
            .attachments(&[attachment])?
            .await?;
    }
    Ok(())
}

pub(crate) async fn handle_interaction(
    app_id: Id<ApplicationMarker>,
    client: Arc<Client>,
    server: Arc<ServerManager>,
    sender: mpsc::Sender<ServerCommand>,
    interaction: Box<InteractionCreate>,
) -> Result<(), anyhow::Error> {
    if let Some(InteractionData::ApplicationCommand(data)) = interaction.clone().0.data {
        let interaction_client = client.interaction(app_id);
        match data.name.as_str() {
            "start" => {
                if !server.running().await {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":orange_circle: Starting up...".to_string(),
                    )
                    .await;

                    let server_path = env::var("SERVER_JAR_PATH").unwrap();
                    let memory = env::var("SERVER_MEMORY").unwrap().parse().unwrap();
                    let jvm_flags = env::var("JVM_FLAGS").ok();

                    sender
                        .send(ServerCommand::StartServer {
                            config: ServerConfig::new(server_path, memory, jvm_flags),
                        })
                        .await
                        .unwrap();
                } else {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":warning: Server already running".to_string(),
                    )
                    .await;
                }
            }
            "send" => {
                if !server.running().await {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":warning: Server isn't running. Start it with `/start`".to_string(),
                    )
                    .await;
                } else if let Some(cmd) = data
                    .options
                    .into_iter()
                    .find(|option| option.name == "command")
                {
                    if let CommandOptionValue::String(cmd) = cmd.value {
                        respond_to_interaction(
                            interaction_client,
                            interaction.id,
                            &interaction.token,
                            format!("`{cmd}`"),
                        )
                        .await;
                        sender.send(ServerCommand::Stdin(cmd)).await.unwrap();
                    }
                }
            }
            "say" => {
                if !server.running().await {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":warning: Server isn't running. Start it with `/start`".to_string(),
                    )
                    .await;
                } else if let Some(cmd) = data
                    .options
                    .into_iter()
                    .find(|option| option.name == "message")
                {
                    if let CommandOptionValue::String(msg) = cmd.value {
                        let user = client
                            .user(interaction.author_id().unwrap())
                            .await
                            .unwrap()
                            .model()
                            .await
                            .unwrap()
                            .name;

                        respond_to_interaction(
                            interaction_client,
                            interaction.id,
                            &interaction.token,
                            format!("<{user} Discord> {msg}"),
                        )
                        .await;
                        let msg = format!(
                            r##"tellraw @a ["",{{"text":"<{user} "}},{{"text":"Discord","color":"#5865F2"}},{{"text":">","color":"white"}},{{"text":" {msg}"}}]"##
                        );
                        sender.send(ServerCommand::Stdin(msg)).await.unwrap();
                    }
                }
            }
            "stop" => {
                if !server.running().await {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":warning: Server isn't running. Start it with `/start`".to_string(),
                    )
                    .await;
                } else {
                    respond_to_interaction(
                        interaction_client,
                        interaction.id,
                        &interaction.token,
                        ":orange_circle: Stopping the server...".to_string(),
                    )
                    .await;
                    sender
                        .send(ServerCommand::Stdin("stop".to_string()))
                        .await
                        .unwrap();
                }
            }
            _ => {}
        };
    }

    Ok(())
}

pub(crate) async fn set_commands(
    app_id: Id<ApplicationMarker>,
    client: Arc<Client>,
) -> Result<(), anyhow::Error> {
    let commands = [
        CommandBuilder::new(
            "start",
            "Starts the Minecraft server",
            CommandType::ChatInput,
        )
        .build(),
        CommandBuilder::new("stop", "Stops the Minecraft server", CommandType::ChatInput).build(),
        CommandBuilder::new(
            "send",
            "Pass a command to the Minecraft server.",
            CommandType::ChatInput,
        )
        .option(StringBuilder::new("command", "Command to pass to the server.").required(true))
        .build(),
        CommandBuilder::new(
            "say",
            "Pass a message to the ingame chat.",
            CommandType::ChatInput,
        )
        .option(StringBuilder::new("message", "Message to pass to the ingame chat.").required(true))
        .build(),
    ];

    let interaction_client = client.interaction(app_id);

    if env::var("DEV").is_ok() {
        let guild_id = Id::new(
            env::var("GUILD_ID")
                .expect("GUILD_ID env var not found")
                .parse()?,
        );
        interaction_client
            .set_guild_commands(guild_id, &commands)
            .await?;
        info!("Commands set for guild {}", guild_id.to_string());
    } else {
        interaction_client.set_global_commands(&commands).await?;
        info!("Commands set globally");
    }

    Ok(())
}

async fn respond_to_interaction(
    interaction_client: InteractionClient<'_>,
    id: Id<InteractionMarker>,
    token: &str,
    content: String,
) {
    let result = interaction_client
        .create_response(
            id,
            token,
            &InteractionResponse {
                kind: InteractionResponseType::ChannelMessageWithSource,
                data: Some(InteractionResponseData {
                    content: Some(content),
                    ..Default::default()
                }),
            },
        )
        .await;
    if let Err(e) = result {
        warn!("Failed responding to interaction: {}", e);
    }
}

pub(crate) async fn manage_status(
    message_sender: Arc<MessageSender>,
    current_status: ServerStatus,
    max_players: Option<u8>,
    msg: &str,
) -> ServerStatus {
    if current_status == ServerStatus::Offline {
        set_status(message_sender, ServerStatus::Starting).await;
        return ServerStatus::Starting;
    };
    if msg.contains("! For help, type \"help\"") {
        set_status(
            message_sender,
            ServerStatus::Running {
                players: Some(0),
                max_players,
            },
        )
        .await;
        return ServerStatus::Running {
            players: Some(0),
            max_players,
        };
    }
    if let ServerStatus::Running {
        players,
        max_players,
    } = current_status
    {
        if msg.contains("logged in with entity id") && max_players.is_some() {
            set_status(
                message_sender,
                ServerStatus::Running {
                    players: Some(players.unwrap() + 1),
                    max_players,
                },
            )
            .await;
            return ServerStatus::Running {
                players: Some(players.unwrap() + 1),
                max_players,
            };
        }
        if msg.contains(" left the game") && max_players.is_some() {
            set_status(
                message_sender,
                ServerStatus::Running {
                    players: Some(players.unwrap() - 1),
                    max_players,
                },
            )
            .await;
            return ServerStatus::Running {
                players: Some(players.unwrap() - 1),
                max_players,
            };
        }
    }
    if msg.contains("Stopping the server") {
        set_status(message_sender, ServerStatus::Stopping).await;
        return ServerStatus::Stopping;
    }
    if msg.contains(":red_circle: Server stopped") {
        set_status(message_sender, ServerStatus::Offline).await;
        return ServerStatus::Offline;
    }
    current_status
}

pub(crate) async fn set_status(message_sender: Arc<MessageSender>, status: ServerStatus) {
    let request = match status {
        ServerStatus::Offline => {
            let activity = Activity::from(MinimalActivity {
                kind: ActivityType::Playing,
                name: "🔴 Offline".to_owned(),
                url: None,
            });
            UpdatePresence::new(Vec::from([activity]), false, None, Status::Idle).unwrap()
        }
        ServerStatus::Starting => {
            let activity = Activity::from(MinimalActivity {
                kind: ActivityType::Playing,
                name: "🟠 Starting".to_owned(),
                url: None,
            });
            UpdatePresence::new(Vec::from([activity]), false, None, Status::Online).unwrap()
        }
        ServerStatus::Running {
            players,
            max_players,
        } => {
            let activity = Activity::from(MinimalActivity {
                kind: ActivityType::Playing,
                name: if players.is_some() && max_players.is_some() {
                    format!("🟢 Online | {}/{}", players.unwrap(), max_players.unwrap())
                } else {
                    "🟢 Online".to_string()
                },
                url: None,
            });
            UpdatePresence::new(Vec::from([activity]), false, None, Status::Online).unwrap()
        }
        ServerStatus::Stopping => {
            let activity = Activity::from(MinimalActivity {
                kind: ActivityType::Playing,
                name: "🟠 Stopping".to_owned(),
                url: None,
            });
            UpdatePresence::new(Vec::from([activity]), false, None, Status::Online).unwrap()
        }
    };

    if let Err(e) = message_sender.command(&request) {
        warn!("Failed updating discord presence: {}", e);
    }
}
