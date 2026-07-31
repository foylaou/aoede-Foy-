use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::process::env_path;

/// 對應 aoede 的 `Config`（見 src/lib/config.rs），
/// 只是這裡拿掉了讀取端的 default，因為表單本身就會顯示空值。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AoedeConfig {
    pub discord_token: String,
    pub discord_user_id: String,
    pub spotify_bot_autoplay: bool,
    pub spotify_device_name: String,
    pub cache_dir: String,
    pub spotify_auto_reconnect: bool,
}

fn parse_env(content: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

pub fn read_config(app: &AppHandle) -> Result<AoedeConfig, String> {
    let path = env_path(app);
    if !path.exists() {
        return Ok(AoedeConfig::default());
    }

    let content = fs::read_to_string(&path).map_err(|e| format!("讀取 .env 失敗: {e}"))?;
    let map = parse_env(&content);

    Ok(AoedeConfig {
        discord_token: map.get("DISCORD_TOKEN").cloned().unwrap_or_default(),
        discord_user_id: map.get("DISCORD_USER_ID").cloned().unwrap_or_default(),
        spotify_bot_autoplay: map
            .get("SPOTIFY_BOT_AUTOPLAY")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        spotify_device_name: map.get("SPOTIFY_DEVICE_NAME").cloned().unwrap_or_default(),
        cache_dir: map.get("CACHE_DIR").cloned().unwrap_or_default(),
        spotify_auto_reconnect: map
            .get("SPOTIFY_AUTO_RECONNECT")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    })
}

/// 直接整份改寫 .env。v1 先不保留使用者手動加的其他欄位或註解，
/// 之後如果需要保留自訂欄位，可以改成「只更新已知的 key，其餘原樣保留」。
pub fn write_config(app: &AppHandle, config: AoedeConfig) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("# Discord Bot 設定\n");
    out.push_str(&format!("DISCORD_TOKEN={}\n", config.discord_token));
    out.push_str(&format!("DISCORD_USER_ID={}\n", config.discord_user_id));
    out.push_str("\n# Aoede 設定\n");
    out.push_str(&format!("CACHE_DIR={}\n", config.cache_dir));
    out.push_str(&format!(
        "SPOTIFY_BOT_AUTOPLAY={}\n",
        config.spotify_bot_autoplay
    ));
    out.push_str(&format!(
        "SPOTIFY_DEVICE_NAME={}\n",
        config.spotify_device_name
    ));
    out.push_str(&format!(
        "SPOTIFY_AUTO_RECONNECT={}\n",
        config.spotify_auto_reconnect
    ));

    let path = env_path(app);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("無法建立設定目錄 {}: {e}", parent.display()))?;
    }

    fs::write(path, out).map_err(|e| format!("寫入 .env 失敗: {e}"))
}
