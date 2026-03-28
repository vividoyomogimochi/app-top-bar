use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_bar_height")]
    pub bar_height: u32,
    #[serde(default)]
    pub monitor: u32,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_true")]
    pub auto_hide_fullscreen: bool,
    #[serde(default)]
    pub server_command: Option<String>,
}

fn default_bar_height() -> u32 {
    80
}
fn default_true() -> bool {
    true
}
fn default_url() -> String {
    "https://ticker.samoyed.moe/ticker/".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bar_height: default_bar_height(),
            monitor: 0,
            auto_start: true,
            url: default_url(),
            auto_hide_fullscreen: true,
            server_command: None,
        }
    }
}

pub struct ConfigState(pub Mutex<AppConfig>);

pub fn config_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("failed to resolve app data dir");
    fs::create_dir_all(&dir).ok();
    dir.join("config.json")
}

pub fn load_config(app: &tauri::AppHandle) -> AppConfig {
    let path = config_path(app);
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => {
            let config = AppConfig::default();
            save_config(app, &config);
            config
        }
    }
}

pub fn save_config(app: &tauri::AppHandle, config: &AppConfig) {
    let path = config_path(app);
    if let Ok(data) = serde_json::to_string_pretty(config) {
        fs::write(path, data).ok();
    }
}
