use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { language: "en".into() } // Default to English
    }
}

pub fn get_config_path(app: &AppHandle) -> PathBuf {
    // Uses Tauri's path resolution API to find standard OS config dir
    let mut path = app.path().app_config_dir().expect("Failed to resolve app_config_dir");
    
    // Ensure the directory itself exists before writing
    fs::create_dir_all(&path).ok();
    
    path.push("config.json");
    path
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    let path = get_config_path(&app);
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let path = get_config_path(&app);
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}
