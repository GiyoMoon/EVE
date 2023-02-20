use super::{config::ServerConfig, enums::ServerStartError};
use log::info;
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Write},
    path::Path,
    process::{ExitStatus, Stdio},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{self, Child},
    sync::broadcast,
};

pub(super) struct ServerInternal {
    pub(super) stdin: process::ChildStdin,
}

impl ServerInternal {
    pub(super) async fn launch(
        config: &ServerConfig,
        stdout_sender: broadcast::Sender<String>,
    ) -> Result<(Self, Child), ServerStartError> {
        config.validate()?;

        let folder = config
            .path
            .as_path()
            .parent()
            .map(|p| p.as_os_str())
            .unwrap_or_else(|| OsStr::new("."));

        let server_jar = config
            .path
            .file_name()
            .expect("Failed getting file name of server jar");

        let args = format!(
            "-Xms{}M -Xmx{}M -jar {} {} nogui",
            config.memory,
            config.memory,
            server_jar.to_str().unwrap_or(""),
            config.jvm_flags.as_deref().unwrap_or(""),
        );

        let eula_path = &format!("{}/eula.txt", folder.to_str().unwrap_or("."));

        if config.auto_accept_eula
            && (!Path::new(eula_path).exists()
                || !fs::read_to_string(eula_path)
                    .unwrap_or_else(|err| panic!("Failed reading {eula_path}: {err}"))
                    .contains("eula=true"))
        {
            info!("Accepting eula");
            stdout_sender
                .send(":green_circle: Accepting eula".to_string())
                .expect("Failed sending value over sender");

            let mut eula_file = File::create(eula_path)?;
            eula_file.write_all(b"eula=true")?;
        }

        let mut child = process::Command::new("java")
            .current_dir(folder)
            .args(args.split(' ').collect::<Vec<&str>>())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .expect("Failed getting stdin of minecraft process");

        Ok((Self { stdin }, child))
    }

    pub(super) async fn run(
        mut process: Child,
        stdout_sender: broadcast::Sender<String>,
    ) -> io::Result<ExitStatus> {
        let mut stdout = BufReader::new(
            process
                .stdout
                .take()
                .expect("Failed getting stdout of minecraft process"),
        )
        .lines();
        let mut stderr = BufReader::new(
            process
                .stderr
                .take()
                .expect("Failed getting stderr of minecraft process"),
        )
        .lines();

        let await_process = tokio::spawn(async move { process.wait().await });

        let stdout_sender_clone = stdout_sender.clone();
        let stderr_handle = tokio::spawn(async move {
            while let Some(line) = stderr
                .next_line()
                .await
                .expect("Failed reading line from stderr of minecraft process")
            {
                stdout_sender_clone
                    .send(line)
                    .expect("Failed sending value over sender");
            }
        });
        let stdout_handle = tokio::spawn(async move {
            while let Some(line) = stdout
                .next_line()
                .await
                .expect("Failed reading line from stdout of minecraft process")
            {
                stdout_sender
                    .send(line)
                    .expect("Failed sending value over sender");
            }
        });

        let (status, _, _) = tokio::join!(await_process, stderr_handle, stdout_handle);

        status.unwrap_or_else(|err| panic!("Failed joining tokio task: {err}"))
    }
}
