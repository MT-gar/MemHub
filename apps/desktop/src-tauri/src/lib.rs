//! Thin Tauri shell around `memhub-core`.
//!
//! - One generic `rpc` command forwards to `memhub_core::api::dispatch`, so the
//!   web UI uses exactly the same command names in the desktop app and in the
//!   browser mode served by the CLI.
//! - A background watcher keeps the vault in sync and emits `memhub://sync`.
//! - `MemHub mcp` (the app binary itself with the `mcp` argument) runs the MCP
//!   server headlessly, so users don't strictly need the sidecar on PATH.

use memhub_core::{api, Hub};
use serde_json::Value;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

struct WatchGuard(#[allow(dead_code)] memhub_core::watch::WatchHandle);

#[tauri::command]
async fn rpc(hub: State<'_, Arc<Hub>>, cmd: String, params: Option<Value>) -> Result<Value, String> {
    let hub = hub.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        api::dispatch(&hub, &cmd, params.unwrap_or(Value::Null)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Absolute path of the bundled `memhub` CLI sidecar (used in MCP snippets).
#[tauri::command]
fn sidecar_path() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no parent dir")?;
    let name = if cfg!(windows) { "memhub.exe" } else { "memhub" };
    let p = dir.join(name);
    if p.exists() {
        Ok(p.to_string_lossy().to_string())
    } else {
        // Fall back to the app binary itself, which also understands `mcp`.
        Ok(exe.to_string_lossy().to_string())
    }
}

#[tauri::command]
fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

pub fn run() {
    // Headless MCP mode: `MemHub mcp` / `memhub-desktop mcp`
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "mcp").unwrap_or(false) {
        let hub = Arc::new(Hub::open().expect("open MemHub home"));
        let h = hub.clone();
        std::thread::spawn(move || {
            let _ = h.sync();
        });
        memhub_core::mcp::McpServer::new(hub, None)
            .run_stdio()
            .expect("MCP server failed");
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let hub = Arc::new(Hub::open()?);
            let handle = app.handle().clone();
            let watch = memhub_core::watch::spawn(
                hub.clone(),
                300,
                Some(Arc::new(move |r: &memhub_core::SyncReport| {
                    let _ = handle.emit("memhub://sync", r);
                })),
            )?;
            let h = hub.clone();
            std::thread::spawn(move || {
                let _ = h.sync();
            });
            app.manage(hub);
            app.manage(WatchGuard(watch));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc, sidecar_path, open_path])
        .run(tauri::generate_context!())
        .expect("error while running MemHub");
}
