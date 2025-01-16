use serde::Deserialize;
use tokio::fs::read_to_string;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(alias = "url")]
    upstream: String,
    #[serde(alias = "leveldb")]
    cache_directory: String,
    #[serde(alias = "expire-time")]
    expire_time: u64,

    #[serde(alias = "bind-address")]
    bind: String,
    //prefix: Option<String>,
}

impl Config {
    pub fn upstream(&self) -> &str {
        &self.upstream
    }

    pub fn cache_directory(&self) -> &str {
        &self.cache_directory
    }

    pub async fn try_read(path: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(&read_to_string(path).await?)?)
    }

    /* pub fn prefix(&self) -> Option<&String> {
        self.prefix.as_ref()
    } */

    pub fn expire_time(&self) -> u64 {
        self.expire_time
    }

    pub fn bind(&self) -> &str {
        &self.bind
    }
}
