use super::{enums::ServerCommand, internal::ServerInternal};
use log::{info, warn};
use std::sync::Arc;
use tokio::{
    io::AsyncWriteExt,
    sync::{mpsc, Mutex},
};

pub(crate) struct ServerManager {
    internal: Arc<Mutex<Option<ServerInternal>>>,
}

impl ServerManager {
    pub(crate) fn new() -> (
        Arc<Self>,
        mpsc::Sender<ServerCommand>,
        mpsc::Receiver<String>,
    ) {
        let (cmd_sender, cmd_receiver) = mpsc::channel::<ServerCommand>(64);
        let (event_sender, event_receiver) = mpsc::channel(64);

        let server = Arc::new(ServerManager {
            internal: Arc::new(Mutex::new(None)),
        });

        server.clone().spawn_listener(event_sender, cmd_receiver);

        (server, cmd_sender, event_receiver)
    }

    fn spawn_listener(
        self: Arc<Self>,
        event_sender: mpsc::Sender<String>,
        mut cmd_receiver: mpsc::Receiver<ServerCommand>,
    ) {
        tokio::spawn(async move {
            while let Some(cmd) = cmd_receiver.recv().await {
                match cmd {
                    ServerCommand::Stdin(cmd) => {
                        self.write_to_stdin(cmd + "\n").await;
                    }
                    ServerCommand::StartServer { config } => {
                        if self.running().await {
                            continue;
                        }
                        info!("Minecraft server started");
                        let event_sender_clone = event_sender.clone();
                        let child =
                            match ServerInternal::launch(&config, event_sender.clone()).await {
                                Ok((internal, child)) => {
                                    *self.internal.lock().await = Some(internal);
                                    child
                                }
                                Err(e) => {
                                    event_sender_clone
                                        .send(format!("Failed to start server: {e}"))
                                        .await
                                        .expect("Failed sending value over sender");
                                    continue;
                                }
                            };

                        let sender = event_sender.clone();
                        let internal_clone = self.internal.clone();

                        tokio::spawn(async move {
                            let run_result = ServerInternal::run(child, sender.clone()).await;

                            if let Err(err) = run_result {
                                warn!("Minecraft process wasn't running: {err}");
                            }

                            let _ = internal_clone.lock().await.take();

                            info!("Minecraft server stopped");

                            sender
                                .clone()
                                .send(":red_circle: Server stopped".to_string())
                                .await
                                .expect("Failed sending value over sender");
                        });
                    }
                }
            }
        });
    }

    async fn write_to_stdin<B: AsRef<[u8]>>(&self, bytes: B) {
        let bytes = bytes.as_ref();
        let mut internal = self.internal.lock().await;
        if let Some(internal) = &mut *internal {
            if let Err(e) = internal.stdin.write_all(bytes).await {
                warn!("Failed to write to Minecraft server stdin: {e}");
            }
        }
    }

    pub(crate) async fn running(&self) -> bool {
        let running = self.internal.lock().await;
        running.is_some()
    }
}
