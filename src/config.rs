use std::env::{self, VarError};

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
}

impl Config {
    pub fn from_env() -> Result<Self, VarError> {
        Ok(Self {
            discord_token: env::var("DISCORD_TOKEN")?,
        })
    }
}
