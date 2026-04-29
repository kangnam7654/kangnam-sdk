mod cli;
mod commands;
mod db;
mod error;
mod mcp;
mod rpc;
mod server;
mod skills;
mod state;

use std::sync::Arc;
use state::AppState;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new().expect("Failed to initialize app state");
    let shared_state = Arc::new(app_state);

    // Start Axum WebSocket server
    let server_state = shared_state.clone();
    let port: u16 = std::env::var("KANGNAM_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = server::start_server(server_state, port).await {
                eprintln!("[server] Failed to start: {}", e);
            }
        });
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance())
        .manage(shared_state)
        .invoke_handler(tauri::generate_handler![
            // Settings (still via Tauri invoke — not yet migrated to RPC)
            commands::settings::settings_get,
            commands::settings::settings_set,
            // Conversations
            commands::conv::conv_list,
            commands::conv::conv_create,
            commands::conv::conv_delete,
            commands::conv::conv_get_messages,
            commands::conv::conv_update_title,
            commands::conv::conv_toggle_pin,
            commands::conv::conv_delete_all,
            commands::conv::conv_export,
            commands::conv::conv_search,
            // Skills / Prompts CRUD
            commands::skills::prompts_list,
            commands::skills::prompts_get,
            commands::skills::prompts_get_instructions,
            commands::skills::prompts_create,
            commands::skills::prompts_update,
            commands::skills::prompts_delete,
            commands::skills::prompts_ref_list,
            commands::skills::prompts_ref_add,
            commands::skills::prompts_ref_update,
            commands::skills::prompts_ref_delete,
            // MCP
            commands::mcp::mcp_list_servers,
            commands::mcp::mcp_add_server,
            commands::mcp::mcp_reconnect_server,
            commands::mcp::mcp_update_server,
            commands::mcp::mcp_remove_server,
            commands::mcp::mcp_list_tools,
            commands::mcp::mcp_server_status,
            commands::mcp::mcp_get_config,
            commands::mcp::mcp_ai_assist,
            // Agents
            commands::agents::agents_list,
            commands::agents::agents_get,
            commands::agents::agents_create,
            commands::agents::agents_update,
            commands::agents::agents_delete,
            commands::agents::read_agent_description,
            commands::agents::list_claude_agents,
            commands::agents::read_claude_agent,
            commands::agents::write_claude_agent,
            commands::agents::delete_claude_agent,
            commands::agents::list_agent_refs,
            commands::agents::read_agent_ref,
            commands::agents::write_agent_ref,
            commands::agents::delete_agent_ref,
            commands::agents::snapshot_agent,
            commands::agents::list_agent_snapshots,
            commands::claude_commands::list_claude_commands,
            commands::claude_commands::read_claude_command,
            commands::claude_commands::write_claude_command,
            commands::claude_commands::delete_claude_command,
            commands::claude_commands::fork_plugin_skill,
            commands::claude_commands::list_skill_refs,
            commands::claude_commands::read_skill_ref,
            commands::claude_commands::write_skill_ref,
            commands::claude_commands::delete_skill_ref,
            commands::claude_commands::snapshot_skill,
            commands::claude_commands::list_skill_snapshots,
        ])
        .setup(|app| {
            // ── System tray ──
            let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        if let Some(win) = tray.app_handle().get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── Restore window state ──
            if let Some(win) = app.get_webview_window("main") {
                restore_window_state(&win);

                let win_clone = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        save_window_state(&win_clone);
                    }
                });
            }

            // ── DevTools in debug ──
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Single instance plugin — focus existing window if already running
fn tauri_plugin_single_instance() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("single-instance")
        .on_event(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            let _ = (app, event);
        })
        .build()
}

// ── Window State Persistence ──

#[derive(serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn window_state_path() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    let base = dirs::config_dir().unwrap_or_default().join("kangnam-client");
    #[cfg(target_os = "windows")]
    let base = dirs::data_dir().unwrap_or_default().join("kangnam-client");
    #[cfg(target_os = "linux")]
    let base = dirs::config_dir().unwrap_or_default().join("kangnam-client");
    base.join("window-state.json")
}

fn save_window_state(win: &tauri::WebviewWindow) {
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
        let state = WindowState {
            x: pos.x as f64,
            y: pos.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        };
        if let Ok(json) = serde_json::to_string(&state) {
            let _ = std::fs::write(window_state_path(), json);
        }
    }
}

fn restore_window_state(win: &tauri::WebviewWindow) {
    let path = window_state_path();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(state) = serde_json::from_str::<WindowState>(&raw) {
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: state.x as i32,
                y: state.y as i32,
            }));
            let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: state.width as u32,
                height: state.height as u32,
            }));
        }
    }
}
