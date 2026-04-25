//! Application state persistence commands.

use std::path::PathBuf;

use diffcore_core::config::DiffcoreConfig;

use super::CommandError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
struct AppStateSnapshotFile {
    version: String,
    saved_at_epoch_ms: u128,
    snapshot: serde_json::Value,
}

#[allow(dead_code)]
fn app_logs_dir() -> Result<PathBuf, CommandError> {
    if let Some(global_config) = DiffcoreConfig::global_config_path() {
        let config_dir = global_config
            .parent()
            .ok_or_else(|| CommandError::Io("Failed to resolve config directory".to_string()))?;
        return Ok(config_dir.join("logs"));
    }

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CommandError::Io("Cannot determine HOME for log directory".to_string()))?;
    Ok(PathBuf::from(home).join(".diffcore").join("logs"))
}

#[allow(dead_code)]
fn app_state_snapshot_dir() -> Result<PathBuf, CommandError> {
    Ok(app_logs_dir()?.join("app-state"))
}

#[tauri::command]
pub fn save_app_state(snapshot: serde_json::Value) -> Result<String, CommandError> {
    // TODO: re-enable app state save/restore after UX and reliability pass.
    let _ = snapshot;
    Err(CommandError::Analysis(
        "App state save/restore is temporarily disabled".to_string(),
    ))

    // let dir = app_state_snapshot_dir()?;
    // std::fs::create_dir_all(&dir)
    //     .map_err(|e| CommandError::Io(format!("Failed to create app-state dir: {}", e)))?;

    // let now = std::time::SystemTime::now()
    //     .duration_since(std::time::UNIX_EPOCH)
    //     .map_err(|e| CommandError::Io(format!("System clock error: {}", e)))?;
    // let saved_at_epoch_ms = now.as_millis();

    // let payload = AppStateSnapshotFile {
    //     version: "1".to_string(),
    //     saved_at_epoch_ms,
    //     snapshot,
    // };

    // let latest_path = dir.join("latest.json");
    // let archive_path = dir.join(format!("snapshot-{}.json", saved_at_epoch_ms));
    // let json = serde_json::to_string_pretty(&payload)
    //     .map_err(|e| CommandError::Io(format!("Failed to serialize app state: {}", e)))?;

    // std::fs::write(&latest_path, &json)
    //     .map_err(|e| CommandError::Io(format!("Failed to write latest app state: {}", e)))?;
    // std::fs::write(&archive_path, json)
    //     .map_err(|e| CommandError::Io(format!("Failed to write archived app state: {}", e)))?;

    // Ok(latest_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn load_last_app_state() -> Result<Option<serde_json::Value>, CommandError> {
    // TODO: re-enable app state save/restore after UX and reliability pass.
    Err(CommandError::Analysis(
        "App state save/restore is temporarily disabled".to_string(),
    ))

    // let latest_path = app_state_snapshot_dir()?.join("latest.json");
    // if !latest_path.exists() {
    //     return Ok(None);
    // }

    // let raw = std::fs::read_to_string(&latest_path)
    //     .map_err(|e| CommandError::Io(format!("Failed to read latest app state: {}", e)))?;
    // let payload: AppStateSnapshotFile = serde_json::from_str(&raw)
    //     .map_err(|e| CommandError::Io(format!("Failed to parse latest app state: {}", e)))?;

    // Ok(Some(payload.snapshot))
}
