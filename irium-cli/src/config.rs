use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub openrouter_api_key: Option<String>,
    pub openrouter_model: Option<String>,
}

pub fn load_app_config() -> Result<AppConfig, String> {
    let Some(path) = config_path() else {
        return Ok(AppConfig::default());
    };
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read config {}: {e}", path.display()))?;
    toml::from_str::<AppConfig>(&raw)
        .map_err(|e| format!("Could not parse config {}: {e}", path.display()))
}

pub fn save_app_config(config: &AppConfig) -> Result<PathBuf, String> {
    let Some(path) = config_path() else {
        return Err("Could not resolve configuration directory".to_string());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Could not create config directory {}: {e}",
                parent.display()
            )
        })?;
    }

    let toml =
        toml::to_string_pretty(config).map_err(|e| format!("Could not serialize config: {e}"))?;
    fs::write(&path, toml)
        .map_err(|e| format!("Could not write config {}: {e}", path.display()))?;
    set_secure_permissions(&path)?;
    Ok(path)
}

pub fn config_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("irium").join("config.toml"))
}

pub fn load_prompt_history() -> Result<Vec<String>, String> {
    let Some(path) = prompt_history_path() else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read prompt history {}: {e}", path.display()))?;
    let entries = serde_json::from_str::<Vec<String>>(&raw)
        .map_err(|e| format!("Could not parse prompt history {}: {e}", path.display()))?;
    Ok(entries
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect())
}

pub fn save_prompt_history(entries: &[String]) -> Result<PathBuf, String> {
    let Some(path) = prompt_history_path() else {
        return Err("Could not resolve prompt history directory".to_string());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Could not create prompt history directory {}: {e}",
                parent.display()
            )
        })?;
    }

    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Could not serialize prompt history: {e}"))?;
    fs::write(&path, json)
        .map_err(|e| format!("Could not write prompt history {}: {e}", path.display()))?;
    Ok(path)
}

pub fn prompt_history_path() -> Option<PathBuf> {
    let base = dirs::data_local_dir().or_else(dirs::data_dir)?;
    Some(base.join("irium").join("prompt_history.json"))
}

#[cfg(unix)]
fn set_secure_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms).map_err(|e| {
        format!(
            "Could not set secure permissions on {}: {e}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_secure_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
