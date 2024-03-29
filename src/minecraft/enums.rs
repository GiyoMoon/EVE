use super::config::ServerConfig;
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ServerConfigError {
    #[error("Server path {0} does not point to a file.")]
    InvalidPath(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) enum ServerCommand {
    Stdin(String),
    StartServer { config: ServerConfig },
    Backup,
}

#[derive(Error, Debug)]
pub(crate) enum ServerStartError {
    #[error("config error: {0}")]
    ConfigError(#[from] ServerConfigError),
    #[error("io error: {0}")]
    IoError(#[from] io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServerStatus {
    Offline,
    Starting,
    Running {
        players: u8,
        max_players: Option<u8>,
    },
    Stopping,
}
