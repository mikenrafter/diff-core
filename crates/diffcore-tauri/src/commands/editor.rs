//! Editor integration and file-save commands.

use std::path::PathBuf;

use super::CommandError;

#[cfg(target_os = "macos")]
fn macos_app_name(editor: &str) -> Option<&'static str> {
    match editor {
        "vscode" => Some("Visual Studio Code"),
        "cursor" => Some("Cursor"),
        "zed" => Some("Zed"),
        _ => None,
    }
}

/// Check if a macOS .app bundle exists in /Applications or ~/Applications.
#[cfg(target_os = "macos")]
fn macos_app_exists(app_name: &str) -> bool {
    let global = format!("/Applications/{}.app", app_name);
    if PathBuf::from(&global).exists() {
        return true;
    }
    if let Ok(home) = std::env::var("HOME") {
        let user = format!("{}/Applications/{}.app", home, app_name);
        if PathBuf::from(&user).exists() {
            return true;
        }
    }
    false
}

/// Open a file in an external editor.
///
/// On macOS, uses `open -a "App Name"` for GUI editors (works without PATH).
/// Falls back to CLI binary for non-macOS or terminal-based editors.
#[tauri::command]
pub fn open_in_editor(editor: String, file_path: String) -> Result<(), CommandError> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(CommandError::Io(format!("File not found: {}", file_path)));
    }

    let result = match editor.as_str() {
        "vscode" | "cursor" | "zed" => {
            #[cfg(target_os = "macos")]
            {
                // Use the CLI binary via the app bundle's bin/ path for proper workspace trust.
                // `open -a` opens files as untrusted; the CLI opens in the existing workspace.
                let cli_path = match editor.as_str() {
                    "vscode" => {
                        "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"
                    }
                    "cursor" => "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
                    "zed" => "/Applications/Zed.app/Contents/MacOS/cli",
                    _ => unreachable!(),
                };
                if std::path::Path::new(cli_path).exists() {
                    std::process::Command::new(cli_path)
                        .args(["--reuse-window", "--goto", &file_path])
                        .spawn()
                } else {
                    // Fallback to `open -a` if CLI path not found
                    let app_name = macos_app_name(&editor).unwrap();
                    std::process::Command::new("open")
                        .args(["-a", app_name, &file_path])
                        .spawn()
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let bin = match editor.as_str() {
                    "vscode" => "code",
                    "cursor" => "cursor",
                    "zed" => "zed",
                    _ => unreachable!(),
                };
                std::process::Command::new(bin)
                    .args(["--reuse-window", "--goto", &file_path])
                    .spawn()
            }
        }
        "vim" => {
            #[cfg(target_os = "macos")]
            {
                // Open vim in a NEW Terminal window via AppleScript
                let escaped = file_path.replace('\\', "\\\\").replace('"', "\\\"");
                std::process::Command::new("osascript")
                    .args([
                        "-e",
                        &format!(
                            "tell application \"Terminal\"\n\
                                activate\n\
                                do script \"vim \\\"{}\\\"\" \n\
                            end tell",
                            escaped
                        ),
                    ])
                    .spawn()
            }
            #[cfg(not(target_os = "macos"))]
            {
                std::process::Command::new("vim").arg(&file_path).spawn()
            }
        }
        "terminal" => {
            let dir = if path.is_dir() {
                file_path.clone()
            } else {
                path.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_path.clone())
            };
            #[cfg(target_os = "macos")]
            {
                // Use AppleScript to open Terminal and cd to the directory
                let escaped = dir.replace('\\', "\\\\").replace('"', "\\\"");
                std::process::Command::new("osascript")
                    .args([
                        "-e",
                        &format!(
                            "tell application \"Terminal\"\n\
                                activate\n\
                                do script \"cd \\\"{}\\\"\" \n\
                            end tell",
                            escaped
                        ),
                    ])
                    .spawn()
            }
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open").arg(&dir).spawn()
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/c", "start", "cmd", "/k", &format!("cd /d {}", dir)])
                    .spawn()
            }
        }
        other => {
            return Err(CommandError::Io(format!("Unknown editor: {}", other)));
        }
    };

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let label = match editor.as_str() {
                "vscode" => "VS Code",
                "cursor" => "Cursor",
                "zed" => "Zed",
                "vim" => "Vim",
                "terminal" => "Terminal",
                _ => &editor,
            };
            Err(CommandError::Io(format!(
                "Failed to open {} — is it installed? ({})",
                label, e
            )))
        }
    }
}

/// Check which editors are available on the system.
///
/// On macOS, checks for .app bundles in /Applications (works without PATH).
/// On other platforms, uses `which`/`where` to find CLI binaries.
#[tauri::command]
pub fn check_editors_available() -> std::collections::HashMap<String, bool> {
    let mut result = std::collections::HashMap::new();

    // GUI editors
    for id in &["vscode", "cursor", "zed"] {
        let available = {
            #[cfg(target_os = "macos")]
            {
                macos_app_name(id)
                    .map(|name| macos_app_exists(name))
                    .unwrap_or(false)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let bin = match *id {
                    "vscode" => "code",
                    "cursor" => "cursor",
                    "zed" => "zed",
                    _ => id,
                };
                #[cfg(unix)]
                {
                    std::process::Command::new("which")
                        .arg(bin)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                }
                #[cfg(windows)]
                {
                    std::process::Command::new("where")
                        .arg(bin)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                }
            }
        };
        result.insert(id.to_string(), available);
    }

    // vim — check binary in PATH (available on most systems)
    let vim_available = {
        #[cfg(unix)]
        {
            std::process::Command::new("which")
                .arg("vim")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        #[cfg(windows)]
        {
            std::process::Command::new("where")
                .arg("vim")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    };
    result.insert("vim".to_string(), vim_available);

    // Terminal is always available
    result.insert("terminal".to_string(), true);

    result
}

/// Persist edited file content to disk.
///
/// Failure modes:
/// - Returns IO error when the path does not exist or is a directory.
/// - Returns IO error when the parent directory is missing.
/// - Returns IO error when the write fails (permissions, disk full, etc).
#[tauri::command]
pub fn save_file_content(file_path: String, content: String) -> Result<(), CommandError> {
    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(CommandError::Io(format!("File not found: {}", file_path)));
    }
    if !path.is_file() {
        return Err(CommandError::Io(format!("Path is not a file: {}", file_path)));
    }
    let parent = path.parent().ok_or_else(|| {
        CommandError::Io(format!("Cannot determine parent directory for: {}", file_path))
    })?;
    if !parent.exists() {
        return Err(CommandError::Io(format!(
            "Parent directory does not exist: {}",
            parent.display()
        )));
    }

    std::fs::write(&path, content)
        .map_err(|e| CommandError::Io(format!("Failed to write file '{}': {}", file_path, e)))
}

