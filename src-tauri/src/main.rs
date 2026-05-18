#![windows_subsystem = "windows"]

mod http;
mod mcp;
mod state;
mod tray;

use state::StateManager;
use state::StateChangeEvent;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::{Emitter, Manager};

#[derive(Clone, serde::Serialize)]
struct VisibilityEvent {
    visible: bool,
}

#[tokio::main]
async fn main() {
    let state_manager = Arc::new(Mutex::new(StateManager::new()));
    let (tx, _rx) = broadcast::channel::<StateChangeEvent>(32);
    let (vis_tx, _vis_rx) = broadcast::channel::<VisibilityEvent>(4);

    let sm_http = state_manager.clone();
    let tx_http = tx.clone();
    let vis_tx_http = vis_tx.clone();

    // Spawn HTTP server on :9527
    tokio::spawn(async move {
        let app = http::create_router(sm_http, tx_http, vis_tx_http);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9527").await.unwrap();
        println!("HTTP server listening on http://127.0.0.1:9527");
        axum::serve(listener, app).await.unwrap();
    });

    let sm_mcp = state_manager.clone();
    let tx_mcp = tx.clone();

    // Spawn MCP server on :9528
    tokio::spawn(async move {
        let app = mcp::create_mcp_router(sm_mcp, tx_mcp);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9528").await.unwrap();
        println!("MCP server listening on http://127.0.0.1:9528");
        axum::serve(listener, app).await.unwrap();
    });

    // Build Tauri app
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_drag, open_obsidian])
        .setup(move |app| {
            // Listen to broadcast channel, forward state changes to frontend
            let handle = app.handle().clone();
            let mut rx = tx.subscribe();
            let handle2 = handle.clone();
            tokio::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    let _ = handle2.emit("state-change", event);
                }
            });

            // Listen to visibility events, toggle window
            let handle3 = handle.clone();
            let mut vis_rx = vis_tx.subscribe();
            tokio::spawn(async move {
                while let Ok(vis_event) = vis_rx.recv().await {
                    if let Some(window) = handle3.get_webview_window("aemeath") {
                        if vis_event.visible {
                            let _ = window.show();
                        } else {
                            let _ = window.hide();
                        }
                    }
                }
            });

            // Send initial waving state
            let _ = handle.emit(
                "state-change",
                StateChangeEvent {
                    animation: "waving".to_string(),
                    bubble: "爱弥斯已上线~".to_string(),
                },
            );

            // Enable system tray
            if let Err(e) = tray::setup(app) {
                eprintln!("Failed to setup tray: {}", e);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Aemeath Pet");
}

#[tauri::command]
fn start_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

#[tauri::command]
fn open_obsidian() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", "obsidian://open?vault=Obsidian%20Vault"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}
