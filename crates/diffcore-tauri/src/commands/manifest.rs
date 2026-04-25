//! Groups manifest import, export, and watch commands.

use std::path::PathBuf;

use tauri::Emitter;

use diffcore_core::types::AnalysisOutput;

use super::{AppState, CommandError};

/// Import a groups manifest JSON and apply it to the current analysis.
///
/// Returns the updated `AnalysisOutput` with groups replaced by the manifest.
#[tauri::command]
pub fn import_groups_manifest(
    manifest_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<AnalysisOutput, CommandError> {
    use diffcore_core::manifest;

    let manifest = manifest::read_manifest(std::path::Path::new(&manifest_path))
        .map_err(|e| CommandError::Io(e))?;

    let analysis = state
        .last_analysis
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?
        .clone()
        .ok_or_else(|| CommandError::Analysis("No analysis loaded".to_string()))?;

    let updated = manifest::import_manifest(&analysis, &manifest);

    // Update cached analysis
    if let Ok(mut last) = state.last_analysis.lock() {
        *last = Some(updated.clone());
    }

    Ok(updated)
}

/// Export the current analysis groups as a manifest JSON file.
#[tauri::command]
pub fn export_groups_manifest(
    output_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    use diffcore_core::manifest;

    let analysis = state
        .last_analysis
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?
        .clone()
        .ok_or_else(|| CommandError::Analysis("No analysis loaded".to_string()))?;

    let groups_manifest = manifest::export_manifest(&analysis);
    manifest::write_manifest(std::path::Path::new(&output_path), &groups_manifest)
        .map_err(|e| CommandError::Io(e))?;

    Ok(())
}

/// Start watching a manifest file for changes. Emits "manifest-changed" events
/// to the frontend when the file is modified.
#[tauri::command]
pub fn watch_manifest(
    manifest_path: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    // Store the path for the watcher
    if let Ok(mut path) = state.watched_manifest_path.lock() {
        *path = Some(PathBuf::from(&manifest_path));
    }

    // Spawn a background thread that polls the file for changes
    let path = PathBuf::from(manifest_path);
    std::thread::spawn(move || {
        let mut last_modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));

            let current_modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();

            if current_modified != last_modified && current_modified.is_some() {
                last_modified = current_modified;
                // Emit event to frontend
                let _ = app_handle.emit("manifest-changed", &path.to_string_lossy().to_string());
            }
        }
    });

    Ok(())
}

/// Stop watching the manifest file.
#[tauri::command]
pub fn unwatch_manifest(state: tauri::State<'_, AppState>) -> Result<(), CommandError> {
    if let Ok(mut path) = state.watched_manifest_path.lock() {
        *path = None;
    }
    Ok(())
}
