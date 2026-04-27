use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use rusqlite::Connection;

use chat_agent::CliManager;
use chat_server::broadcast::{self, BroadcastTx, EnhancedBroadcastTx};

use crate::db;
use crate::mcp::bridge::McpBridge;

pub struct AppState {
    pub db: Arc<StdMutex<Connection>>,
    pub cli_manager: Arc<tokio::sync::Mutex<CliManager>>,
    pub mcp: McpBridge,
    pub broadcast_tx: BroadcastTx,
    pub enhanced_broadcast_tx: EnhancedBroadcastTx,
    pub pending_permissions: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl AppState {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = get_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("kangnam-client.db");
        let mut conn = db::connection::open_database(&db_path)?;
        db::schema::run_migrations(&mut conn)?;

        let mut cli_manager = CliManager::new();
        let port: u16 = std::env::var("KANGNAM_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3001);
        cli_manager.register_adapter(Box::new(
            crate::cli::adapters::claude::ClaudeAdapter::with_port(port),
        ));
        cli_manager.register_adapter(Box::new(
            crate::cli::adapters::codex::CodexAdapter::new(),
        ));

        let (broadcast_tx, _) = broadcast::create_channel();
        let (enhanced_broadcast_tx, _) = broadcast::create_enhanced_channel();

        Ok(Self {
            db: Arc::new(StdMutex::new(conn)),
            cli_manager: Arc::new(tokio::sync::Mutex::new(cli_manager)),
            mcp: McpBridge::new(),
            broadcast_tx,
            enhanced_broadcast_tx,
            pending_permissions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }
}

fn get_data_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let app_dir = get_app_data_dir().ok_or("Could not determine app data directory")?;
    Ok(app_dir.join("data"))
}

fn get_app_data_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::config_dir().map(|p| p.join("kangnam-client"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir().map(|p| p.join("kangnam-client"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::config_dir().map(|p| p.join("kangnam-client"))
    }
}
