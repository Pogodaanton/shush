use std::path::PathBuf;
use directories::ProjectDirs;
use librespot::discovery::Credentials;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::{Error, ErrorKind};

pub const QUALIFIER: &str = "org";
pub const ORGANISATION: &str = "shush";
pub const APPLICATION: &str = "shush";

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    pub credentials: Option<Credentials>
}

impl Config {
    /// Retrieve path to shush files for user
    pub fn dir_path() -> Option<PathBuf> {
        let project_dirs = ProjectDirs::from(QUALIFIER, ORGANISATION, APPLICATION);
        project_dirs.map(|d| d.config_dir().to_owned())
    }

    /// Retrieve path to roaming shush config
    pub fn path() -> Result<PathBuf, Error> {
        match Self::dir_path().map(|p| p.join("config.toml")) {
            None => Err(Error::new(ErrorKind::Unexpected, "Could not find shush config directory for this OS!")),
            Some(p) => Ok(p),
        }
    }

    pub async fn load_from_file() -> Result<Self, Error> {
        let config_path = Self::path()?;

        log::info!("Loading config from {:?}", config_path);
        let mut file = fs::File::open(&config_path).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;

        let config: Self = toml::from_slice(&contents).map_err(Error::invalid_data)?;
        Ok(config)
    }

    pub async fn save_to_file(&self) -> Result<(), Error> {
        let config_path = Self::path()?;

        log::info!("Saving config to {:?}", config_path);
        let mut file = fs::File::create(&config_path).await?;
        let serialized = toml::to_string_pretty(self).map_err(Error::invalid_data)?;

        file.write_all(serialized.as_bytes()).await?;
        Ok(())
    }
}