use figment::{
    Error, Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(alias = "DISCORD_TOKEN")]
    pub discord_token: String,
    #[serde(alias = "DISCORD_USER_ID")]
    pub discord_user_id: u64,
    #[serde(alias = "SPOTIFY_BOT_AUTOPLAY")]
    #[serde(default = "default_false")]
    pub spotify_bot_autoplay: bool,
    #[serde(alias = "SPOTIFY_DEVICE_NAME")]
    #[serde(default = "default_spotify_device_name")]
    pub spotify_device_name: String,
    #[serde(alias = "CACHE_DIR")]
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
}
fn default_false() -> bool {
    false
}
fn default_spotify_device_name() -> String {
    "PUPU MUSIC BOT".to_string()
}

fn default_cache_dir() -> String {
    "cache".to_string()
}

impl Config {
    pub fn new() -> Result<Self, Box<Error>> {
        let config: Config = Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::raw())
            .extract()?;
        Ok(config)
    }
}
