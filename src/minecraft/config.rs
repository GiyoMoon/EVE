use super::enums::ServerConfigError;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct ServerConfig {
    pub(super) path: PathBuf,
    pub(super) memory: u16,
    pub(super) jvm_flags: Option<String>,
}

impl ServerConfig {
    pub fn new<P: Into<PathBuf>>(server_path: P, memory: u16, jvm_flags: Option<String>) -> Self {
        let path = server_path.into();
        ServerConfig {
            path,
            memory,
            jvm_flags,
        }
    }

    pub fn validate(&self) -> Result<(), ServerConfigError> {
        if !self.path.is_file() {
            return Err(ServerConfigError::InvalidPath(self.path.clone()));
        }
        Ok(())
    }
}
