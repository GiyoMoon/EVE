use super::enums::ServerConfigError;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct ServerConfig {
    pub(super) folder: PathBuf,
    pub(super) jar: PathBuf,
    pub(super) memory: u16,
    pub(super) jvm_flags: Option<String>,
    pub(super) auto_accept_eula: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CustomServerConfig {
    pub(super) folder: PathBuf,
    pub(super) auto_accept_eula: bool,
    pub(super) run_cmd: String,
}

impl ServerConfig {
    pub fn new<P: Into<PathBuf>>(
        folder: P,
        jar: P,
        memory: u16,
        jvm_flags: Option<String>,
        auto_accept_eula: bool,
    ) -> Self {
        Self {
            folder: folder.into(),
            jar: jar.into(),
            memory,
            jvm_flags,
            auto_accept_eula,
        }
    }

    pub fn validate(&self) -> Result<(), ServerConfigError> {
        if !self.folder.is_dir() {
            return Err(ServerConfigError::InvalidPath(self.folder.clone()));
        }
        if !self.folder.join(self.jar.clone()).is_file() {
            return Err(ServerConfigError::InvalidJar(
                self.folder.join(self.jar.clone()),
            ));
        }
        Ok(())
    }
}

impl CustomServerConfig {
    pub fn new<P: Into<PathBuf>>(folder: P, auto_accept_eula: bool, run_cmd: String) -> Self {
        Self {
            folder: folder.into(),
            auto_accept_eula,
            run_cmd,
        }
    }

    pub fn validate(&self) -> Result<(), ServerConfigError> {
        if !self.folder.is_dir() {
            return Err(ServerConfigError::InvalidPath(self.folder.clone()));
        }
        Ok(())
    }
}
