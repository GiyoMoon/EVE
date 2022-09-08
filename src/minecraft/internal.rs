use super::{config::ServerConfig, enums::ServerStartError};
use std::{
    ffi::OsStr,
    io,
    process::{ExitStatus, Stdio},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{self, Child},
    sync::mpsc,
};

pub(super) struct ServerInternal {
    pub(super) stdin: process::ChildStdin,
}

impl ServerInternal {
    pub(super) fn launch(config: &ServerConfig) -> Result<(Self, Child), ServerStartError> {
        config.validate()?;

        let folder = config
            .path
            .as_path()
            .parent()
            .map(|p| p.as_os_str())
            .unwrap_or_else(|| OsStr::new("."));

        let server_jar = config.path.file_name().unwrap();

        let args = format!(
            "-Xms{}M -Xmx{}M -jar {} {} nogui",
            config.memory,
            config.memory,
            server_jar.to_str().unwrap_or(""),
            config.jvm_flags.as_deref().unwrap_or(""),
        );

        let mut child = process::Command::new("java")
            .current_dir(folder)
            .args(args.split(' ').collect::<Vec<&str>>())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();

        Ok((Self { stdin }, child))
    }

    pub(super) async fn run(
        mut process: Child,
        sender: mpsc::Sender<String>,
    ) -> io::Result<ExitStatus> {
        let mut stdout = BufReader::new(process.stdout.take().unwrap()).lines();
        let mut stderr = BufReader::new(process.stderr.take().unwrap()).lines();

        let await_process = tokio::spawn(async move { process.wait().await });

        let sender_c = sender.clone();
        let stderr_handle = tokio::spawn(async move {
            while let Some(line) = stderr.next_line().await.unwrap() {
                sender_c.send(line).await.unwrap();
            }
        });
        let stdout_handle = tokio::spawn(async move {
            while let Some(line) = stdout.next_line().await.unwrap() {
                sender.send(line).await.unwrap();
            }
        });

        let (status, _, _) = tokio::join!(await_process, stderr_handle, stdout_handle);

        status.unwrap()
    }
}
