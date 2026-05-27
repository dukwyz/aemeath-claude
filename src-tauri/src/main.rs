#![windows_subsystem = "windows"]

mod http;
mod mcp;
mod state;
mod tray;

use state::StateManager;
use state::StateChangeEvent;
use state::PetState;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tauri::{Emitter, Manager, State};

#[derive(Clone, serde::Serialize)]
struct VisibilityEvent {
    visible: bool,
}

// Shared state accessible from Tauri commands
struct TauriAppState {
    state: Arc<Mutex<state::StateManager>>,
    tx: broadcast::Sender<state::StateChangeEvent>,
}

#[tokio::main]
async fn main() {
    let state_manager = Arc::new(Mutex::new(StateManager::new()));
    let (tx, _rx) = broadcast::channel::<StateChangeEvent>(32);
    let (vis_tx, _vis_rx) = broadcast::channel::<VisibilityEvent>(4);

    // Shared pending input slot for MCP oneshot channel
    let pending_input: state::SharedPendingInput = Arc::new(Mutex::new(None));

    let sm_http = state_manager.clone();
    let tx_http = tx.clone();
    let vis_tx_http = vis_tx.clone();
    let pending_http = pending_input.clone();

    // Spawn HTTP server on :9527
    tokio::spawn(async move {
        let app = http::create_router(sm_http, tx_http, vis_tx_http, pending_http);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9527").await.unwrap();
        println!("HTTP server listening on http://127.0.0.1:9527");
        axum::serve(listener, app).await.unwrap();
    });

    let sm_mcp = state_manager.clone();
    let tx_mcp = tx.clone();
    let pending_mcp = pending_input.clone();

    // Spawn MCP server on :9528
    tokio::spawn(async move {
        let app = mcp::create_mcp_router(sm_mcp, tx_mcp, pending_mcp);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9528").await.unwrap();
        println!("MCP server listening on http://127.0.0.1:9528");
        axum::serve(listener, app).await.unwrap();
    });

    // Build Tauri app
    let tauri_state = TauriAppState {
        state: state_manager.clone(),
        tx: tx.clone(),
    };

    tauri::Builder::default()
        .manage(tauri_state)
        .invoke_handler(tauri::generate_handler![start_drag, open_obsidian, approve_permission, deny_permission])
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
                    overlay: None,
                    input_type: None,
                    options: None,
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

// ---- Windows FFI for relay_to_terminal ----

#[cfg(target_os = "windows")]
mod winffi {
    extern "system" {
        pub fn EnumWindows(lpEnumFunc: isize, lParam: isize) -> i32;
        pub fn GetWindowTextW(hWnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn GetWindowTextLengthW(hWnd: isize) -> i32;
        pub fn IsWindowVisible(hWnd: isize) -> i32;
        pub fn SetForegroundWindow(hWnd: isize) -> i32;
        pub fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
        pub fn GetForegroundWindow() -> isize;
        pub fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
    }
    pub const SW_RESTORE: i32 = 9;
    pub const KEYEVENTF_KEYUP: u32 = 0x0002;
    pub const VK_CONTROL: u8 = 0x11;
    pub const VK_V: u8 = 0x56;
    pub const VK_RETURN: u8 = 0x0D;
}

fn relay_to_terminal(msg: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // 1. Copy message to clipboard via PowerShell
        let ps_cmd = format!(
            "Set-Clipboard -Value '{}'",
            msg.replace('\'', "''")
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        // 2. Find Claude Code window by title
        use std::sync::Mutex;
        static FOUND_HWND: Mutex<Vec<isize>> = Mutex::new(Vec::new());

        unsafe {
            FOUND_HWND.lock().unwrap().clear();
            extern "system" fn enum_callback(hwnd: isize, _lparam: isize) -> i32 {
                unsafe {
                    let len = winffi::GetWindowTextLengthW(hwnd);
                    if len > 0 && winffi::IsWindowVisible(hwnd) != 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        winffi::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
                        let title = String::from_utf16_lossy(&buf);
                        if title.to_lowercase().contains("claude") {
                            FOUND_HWND.lock().unwrap().push(hwnd);
                        }
                    }
                }
                1 // continue enumeration
            }
            winffi::EnumWindows(enum_callback as *const () as isize, 0);
        }

        let hwnd = {
            let found = FOUND_HWND.lock().unwrap();
            found.first().copied()
        };

        if let Some(hwnd) = hwnd {
            unsafe {
                let prev = winffi::GetForegroundWindow();
                winffi::ShowWindow(hwnd, winffi::SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(50));
                winffi::SetForegroundWindow(hwnd);
                std::thread::sleep(std::time::Duration::from_millis(50));

                // Ctrl+V
                winffi::keybd_event(winffi::VK_CONTROL, 0, 0, 0);
                winffi::keybd_event(winffi::VK_V, 0, 0, 0);
                winffi::keybd_event(winffi::VK_V, 0, winffi::KEYEVENTF_KEYUP, 0);
                winffi::keybd_event(winffi::VK_CONTROL, 0, winffi::KEYEVENTF_KEYUP, 0);
                std::thread::sleep(std::time::Duration::from_millis(30));

                // Enter
                winffi::keybd_event(winffi::VK_RETURN, 0, 0, 0);
                winffi::keybd_event(winffi::VK_RETURN, 0, winffi::KEYEVENTF_KEYUP, 0);
                std::thread::sleep(std::time::Duration::from_millis(30));

                // Restore previous foreground window
                if prev != hwnd {
                    winffi::SetForegroundWindow(prev);
                }
            }
        }
    }
}

#[tauri::command]
async fn approve_permission(app_state: State<'_, TauriAppState>) -> Result<(), String> {
    // Claude Code 权限菜单：输入 3 选择 "Always allow"
    relay_to_terminal("3");
    // Clear Permission state so subsequent hooks can proceed
    clear_permission_state(&app_state).await;
    Ok(())
}

#[tauri::command]
async fn deny_permission(app_state: State<'_, TauriAppState>) -> Result<(), String> {
    // Claude Code 权限菜单：输入 1 选择 "Deny"
    relay_to_terminal("1");
    // Clear Permission state so subsequent hooks can proceed
    clear_permission_state(&app_state).await;
    Ok(())
}

async fn clear_permission_state(app_state: &TauriAppState) {
    let mut mgr = app_state.state.lock().await;
    if mgr.is_force_hidden() {
        return;
    }
    if matches!(mgr.current_state(), PetState::Permission) {
        mgr.set_state(PetState::Idle, None);
    }
    let animation = mgr.current_animation_name();
    let bubble = mgr.current_state().bubble_text(None).to_string();
    drop(mgr);

    let _ = app_state.tx.send(StateChangeEvent {
        animation,
        bubble,
        overlay: None,
        input_type: None,
        options: None,
    });
}
