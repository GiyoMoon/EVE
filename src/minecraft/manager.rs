use super::{enums::ServerCommand, internal::ServerInternal};
use astrolabe::DateTime;
use log::{info, warn};
use std::{env, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    sync::{broadcast, mpsc, Mutex},
};

pub(crate) struct ServerManager {
    internal: Arc<Mutex<Option<ServerInternal>>>,
    stdout_sender: broadcast::Sender<String>,
}

impl ServerManager {
    pub(crate) fn new() -> (
        Arc<Self>,
        mpsc::Sender<ServerCommand>,
        broadcast::Receiver<String>,
    ) {
        let (cmd_sender, cmd_receiver) = mpsc::channel::<ServerCommand>(64);
        let (stdout_sender, stdout_receiver) = broadcast::channel(64);

        let server = Arc::new(ServerManager {
            internal: Arc::new(Mutex::new(None)),
            stdout_sender,
        });

        server.clone().spawn_listener(cmd_receiver);

        (server, cmd_sender, stdout_receiver)
    }

    fn spawn_listener(self: Arc<Self>, mut cmd_receiver: mpsc::Receiver<ServerCommand>) {
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
                        let child =
                            match ServerInternal::launch(&config, self.stdout_sender.clone()).await
                            {
                                Ok((internal, child)) => {
                                    *self.internal.lock().await = Some(internal);
                                    child
                                }
                                Err(e) => {
                                    self.stdout_sender
                                        .send(format!("Failed to start server: {e}"))
                                        .expect("Failed sending value over sender");
                                    continue;
                                }
                            };

                        let stdout_sender_clone = self.stdout_sender.clone();
                        let internal_clone = self.internal.clone();

                        tokio::spawn(async move {
                            let run_result =
                                ServerInternal::run(child, stdout_sender_clone.clone()).await;

                            if let Err(err) = run_result {
                                warn!("Minecraft process wasn't running: {err}");
                            }

                            let _ = internal_clone.lock().await.take();

                            info!("Minecraft server stopped");

                            stdout_sender_clone
                                .send(":red_circle: Server stopped".to_string())
                                .expect("Failed sending value over sender");
                        });
                    }
                    ServerCommand::Backup => {
                        if let Err(backup_err) =
                            self.clone().create_backup(self.stdout_sender.clone()).await
                        {
                            if backup_err.1 {
                                self.enable_save().await;
                            }
                            self.stdout_sender
                                .send(backup_err.0)
                                .expect("Failed sending value over sender");
                        }
                    }
                }
            }
        });
    }

    async fn create_backup(
        self: Arc<ServerManager>,
        stdout_sender: broadcast::Sender<String>,
    ) -> Result<(), (String, bool)> {
        info!("Starting server backup...");
        let self_clone = self.clone();
        let handle = tokio::spawn(async move {
            self_clone
                .await_stdout(
                    "Automatic saving is now disabled".to_string(),
                    Duration::from_secs(10),
                )
                .await
        });
        self.write_to_stdin("save-off\n").await;
        let success = handle.await.expect("Failed joining tokio thread");
        if !success {
            return Err((
                ":warning: Failed creating server backup, `save-off` did not run successfully."
                    .to_string(),
                false,
            ));
        }

        let self_clone = self.clone();
        let handle = tokio::spawn(async move {
            self_clone
                .await_stdout("Saved the game".to_string(), Duration::from_secs(60))
                .await
        });
        self.write_to_stdin("save-all\n").await;
        let success = handle.await.expect("Failed joining tokio thread");
        if !success {
            return Err((
                ":warning: Failed creating server backup, `save-all` did not run successfully."
                    .to_string(),
                true,
            ));
        }

        let server_folder: String = env::var("SERVER_FOLDER").map_err(|_| {
            (
                ":warning: Failed creating server backup, `SERVER_FOLDER` environment variable is not set."
                    .to_string(),
                true,
            )
        })?;

        let backup_folder = env::var("BACKUP_FOLDER").map_err(|_| {
            (
                ":warning: Failed creating server backup, `BACKUP_FOLDER` environment variable is not set."
                    .to_string(),
                true,
            )
        })?;

        let backup_name = DateTime::now().format(
            &env::var("BACKUP_NAME").unwrap_or("'backup'_yyyy_MM_dd_HH_mm'.tar.gz'".to_string()),
        );

        let backup_command = env::var("BACKUP_COMMAND").ok();

        let child_result = if let Some(backup_command) = backup_command {
            let backup_command = backup_command
                .replace("{BACKUP_FOLDER}", &backup_folder)
                .replace("{BACKUP_NAME}", &backup_name)
                .replace("{SERVER_FOLDER}", &server_folder);

            let splitted_command: Vec<&str> = backup_command.split(' ').collect();

            let mut command = Command::new(splitted_command[0]);
            command
                .args(&splitted_command[1..])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        } else {
            let mut command = Command::new("tar");
            command
                .args([
                    "-czf",
                    &format!("{backup_folder}/{backup_name}"),
                    &server_folder,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        };

        child_result
            .map_err(|err| {
                (
                    format!(
                        ":warning: Failed creating server backup, backup command failed: {err}"
                    ),
                    true,
                )
            })?
            .wait()
            .await
            .map_err(|err| {
                (
                    format!(
                        ":warning: Failed creating server backup, backup command failed: {err}"
                    ),
                    true,
                )
            })?;

        stdout_sender
            .send(format!(
                ":white_check_mark: Successfully created server backup `{backup_name}`"
            ))
            .expect("Failed sending value over sender");
        self.enable_save().await;
        info!("Successfully created server backup");

        Ok(())
    }

    async fn enable_save(&self) {
        self.write_to_stdin("save-on\n").await;
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

    async fn await_stdout(&self, expected_msg: String, timeout: Duration) -> bool {
        let mut stdout_receiver = self.stdout_sender.subscribe();

        let message_handle = tokio::spawn(async move {
            while let Ok(msg) = stdout_receiver.recv().await {
                if msg.contains(&expected_msg) {
                    break;
                }
            }
        });

        let timeout_handle = tokio::spawn(async move { tokio::time::sleep(timeout).await });

        let success = tokio::select! {
            _ = message_handle => true,
            _ = timeout_handle => false
        };
        success
    }

    pub(crate) async fn running(&self) -> bool {
        let running = self.internal.lock().await;
        running.is_some()
    }
}
