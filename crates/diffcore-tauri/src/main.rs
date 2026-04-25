// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

mod activity_stream;
mod commands;

use commands::AppState;

/// Set the macOS dock icon programmatically.
/// This is needed because `cargo tauri dev` runs the binary directly (not as a `.app` bundle),
/// so macOS shows a generic "exec" icon in the dock. Setting it via NSApplication ensures
/// the correct icon appears in both dev and production modes.
#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    // Safety: this function is called from the Tauri setup hook which runs on the main thread.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let icon_bytes = include_bytes!("../icons/icon.png");
    let data = NSData::with_bytes(icon_bytes);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}

fn main() {
    if let Err(e) = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            set_macos_dock_icon();
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Core commands (defined in commands/mod.rs)
            commands::analyze,
            commands::get_last_analysis,
            commands::get_mermaid,
            commands::get_file_diff,
            // LLM annotation and refinement (commands/llm.rs)
            commands::llm::start_annotate_overview,
            commands::llm::annotate_overview,
            commands::llm::start_annotate_group,
            commands::llm::annotate_group,
            commands::llm::start_refine_groups,
            commands::llm::refine_groups,
            commands::llm::get_cached_refinement,
            commands::llm::store_refinement_cache,
            // Git / workspace (commands/workspace.rs)
            commands::workspace::list_branches,
            commands::workspace::list_commits,
            commands::workspace::list_worktrees,
            commands::workspace::get_branch_status,
            commands::workspace::get_repo_info,
            commands::workspace::get_launch_directory,
            commands::workspace::get_last_diff_file_statuses,
            commands::workspace::list_repo_path_suggestions,
            commands::workspace::cross_file_search,
            commands::workspace::get_workspace_file_content,
            commands::workspace::parse_file_content,
            // Settings / API keys (commands/settings.rs)
            commands::settings::check_api_key,
            commands::settings::get_llm_settings,
            commands::settings::save_llm_settings,
            commands::settings::save_api_key,
            commands::settings::clear_api_key,
            commands::settings::fetch_provider_models,
            commands::settings::get_ignore_paths,
            commands::settings::save_ignore_paths,
            // Editor integration (commands/editor.rs)
            commands::editor::open_in_editor,
            commands::editor::save_file_content,
            commands::editor::check_editors_available,
            // Review comments (commands/comments.rs)
            commands::comments::save_comment,
            commands::comments::delete_comment,
            commands::comments::load_comments,
            commands::comments::export_comments,
            commands::comments::save_comment_cached,
            commands::comments::load_comments_cached,
            commands::comments::delete_comment_cached,
            commands::comments::update_comment_cached,
            // App state persistence (commands/app_state.rs)
            commands::app_state::save_app_state,
            commands::app_state::load_last_app_state,
            // Groups manifest (commands/manifest.rs)
            commands::manifest::import_groups_manifest,
            commands::manifest::export_groups_manifest,
            commands::manifest::watch_manifest,
            commands::manifest::unwatch_manifest,
        ])
        .run(tauri::generate_context!())
    {
        log::error!("Fatal: diffcore failed to start: {}", e);
        std::process::exit(1);
    }
}
