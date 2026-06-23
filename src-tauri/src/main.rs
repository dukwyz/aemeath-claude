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
    let hwnd_http: Arc<std::sync::Mutex<isize>> = Arc::new(std::sync::Mutex::new(0));

    // Spawn HTTP server on :9527
    tokio::spawn(async move {
        let app = http::create_router(sm_http, tx_http, vis_tx_http, pending_http, hwnd_http);
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
        .invoke_handler(tauri::generate_handler![start_drag, open_obsidian, approve_permission, deny_permission, hide_window, exit_app])
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
    // 先尝试查找已有 Obsidian 窗口并聚焦，避免重复开新窗口
    #[cfg(target_os = "windows")]
    {
        use std::sync::Mutex;
        static OBSIDIAN_HWND: Mutex<Vec<isize>> = Mutex::new(Vec::new());

        unsafe {
            OBSIDIAN_HWND.lock().unwrap().clear();
            extern "system" fn find_obsidian(hwnd: isize, _lparam: isize) -> i32 {
                unsafe {
                    let len = winffi::GetWindowTextLengthW(hwnd);
                    if len > 0 && winffi::IsWindowVisible(hwnd) != 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        winffi::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
                        let title = String::from_utf16_lossy(&buf);
                        let tl = title.to_lowercase();
                        // Obsidian 窗口标题通常包含 "obsidian" 或 vault 名
                        if tl.contains("obsidian") || tl.contains("obsidian vault") {
                            // 排除桌宠自身的窗口（标题含 aemeath）
                            if !tl.contains("aemeath") {
                                OBSIDIAN_HWND.lock().unwrap().push(hwnd);
                            }
                        }
                    }
                }
                1
            }
            winffi::EnumWindows(find_obsidian as *const () as isize, 0);
        }

        let hwnd = OBSIDIAN_HWND.lock().unwrap().first().copied();
        if let Some(hwnd) = hwnd {
            // 找到已有窗口 → 聚焦（SW_RESTORE = 9，处理最小化情况）
            unsafe {
                winffi::ShowWindow(hwnd, 9);
                winffi::SetForegroundWindow(hwnd);
            }
            return;
        }
    }

    // 没找到已有窗口 → 通过 URI 启动 Obsidian
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", "obsidian://open?vault=Obsidian%20Vault"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// ---- Windows FFI for relay_to_terminal ----

#[cfg(target_os = "windows")]
#[allow(clashing_extern_declarations)]
mod winffi {
    extern "system" {
        pub fn EnumWindows(lpEnumFunc: isize, lParam: isize) -> i32;
        pub fn GetWindowTextW(hWnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32;
        pub fn GetWindowTextLengthW(hWnd: isize) -> i32;
        pub fn IsWindowVisible(hWnd: isize) -> i32;
        pub fn SetForegroundWindow(hWnd: isize) -> i32;
        pub fn GetForegroundWindow() -> isize;
        pub fn GetClassNameW(hWnd: isize, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        pub fn OpenClipboard(hwnd: isize) -> i32;
        pub fn CloseClipboard() -> i32;
        pub fn EmptyClipboard() -> i32;
        pub fn SetClipboardData(format: u32, mem: isize) -> isize;
        pub fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
        pub fn GlobalLock(mem: isize) -> *mut u8;
        pub fn GlobalUnlock(mem: isize) -> i32;
        pub fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        pub fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
        pub fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
        pub fn GetCurrentThreadId() -> u32;
        pub fn SendInput(nInputs: u32, pInputs: *const u8, cbSize: i32) -> u32;
    }
    pub const KEYEVENTF_KEYUP: u32 = 0x0002;
    pub const VK_CONTROL: u8 = 0x11;
    pub const VK_V: u8 = 0x56;
    pub const VK_RETURN: u8 = 0x0D;
    pub const GMEM_MOVEABLE: u32 = 0x0002;
    pub const CF_UNICODETEXT: u32 = 13;
    pub const INPUT_KEYBOARD: u32 = 1;
}

/// Write diagnostic log to relay.log (same dir as game_guard.log)
#[cfg(target_os = "windows")]
fn log_relay(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("relay.log")
    {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

/// Copy UTF-16 text to Windows clipboard using raw Win32 API (no PowerShell needed).
/// Returns true on success.
#[cfg(target_os = "windows")]
unsafe fn set_clipboard_utf16(text: &str) -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Encode as null-terminated UTF-16
    let wide: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide.len() * 2;

    let h_mem = winffi::GlobalAlloc(winffi::GMEM_MOVEABLE, byte_len);
    if h_mem == 0 {
        log_relay("  clipboard: GlobalAlloc failed");
        return false;
    }

    let ptr = winffi::GlobalLock(h_mem);
    if ptr.is_null() {
        log_relay("  clipboard: GlobalLock failed");
        return false;
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr, byte_len);
    winffi::GlobalUnlock(h_mem);

    if winffi::OpenClipboard(0) == 0 {
        log_relay("  clipboard: OpenClipboard failed");
        return false;
    }
    winffi::EmptyClipboard();
    let result = winffi::SetClipboardData(winffi::CF_UNICODETEXT, h_mem);
    winffi::CloseClipboard();

    if result == 0 {
        log_relay("  clipboard: SetClipboardData failed");
        return false;
    }
    true
}

/// Send a single virtual key press via SendInput.
/// key: virtual key code, flags: 0 for key-down, KEYEVENTF_KEYUP for key-up.
#[cfg(target_os = "windows")]
unsafe fn send_key_press(key: u8, flags: u32) {
    // Win32 INPUT structure (x64): 40 bytes
    // [0-3]:   type = INPUT_KEYBOARD (1)
    // [4-7]:   padding
    // [8-9]:   wVk
    // [10-11]: wScan (0)
    // [12-15]: dwFlags
    // [16-19]: time (0)
    // [20-23]: padding
    // [24-31]: dwExtraInfo (0)
    let mut input = [0u8; 40];
    input[0..4].copy_from_slice(&winffi::INPUT_KEYBOARD.to_le_bytes());
    input[8..10].copy_from_slice(&key.to_le_bytes());
    input[12..16].copy_from_slice(&flags.to_le_bytes());
    winffi::SendInput(1, input.as_ptr(), std::mem::size_of::<[u8; 40]>() as i32);
}

/// Try to relay `msg` to the Claude Code terminal via clipboard paste.
/// Returns true if paste was attempted (window found), false if no window found.
#[cfg(target_os = "windows")]
fn relay_to_terminal(msg: &str) -> bool {
    use std::sync::Mutex;
    static FOUND_HWND: Mutex<Vec<(isize, bool, bool)>> = Mutex::new(Vec::new());

    log_relay(&format!("relay_to_terminal: msg len={}", msg.len()));

    // 1. Copy message to clipboard (fast: raw Win32 API, no PowerShell)
    let clip_ok = unsafe { set_clipboard_utf16(msg) };
    log_relay(&format!("  clipboard set: {}", if clip_ok { "OK" } else { "FAIL" }));

    // 2. Find Claude Code window by title
    unsafe {
        FOUND_HWND.lock().unwrap().clear();
        extern "system" fn enum_callback(hwnd: isize, _lparam: isize) -> i32 {
            unsafe {
                let len = winffi::GetWindowTextLengthW(hwnd);
                if len > 0 && winffi::IsWindowVisible(hwnd) != 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    winffi::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
                    let title = String::from_utf16_lossy(&buf);

                    if title.to_lowercase().contains("claude") || title.to_lowercase().contains("obsidian") {
                        let mut class_buf = [0u16; 256];
                        winffi::GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
                        let class_name = String::from_utf16_lossy(&class_buf)
                            .trim_matches('\0')
                            .to_lowercase();

                        let is_terminal = class_name.contains("consolewindowclass")
                            || class_name.contains("cascadiamainwindow")
                            || class_name.contains("windows.ui.core.corewindow")
                            || class_name.contains("putty")
                            || class_name.contains("mobaxterm");
                        let is_vscode = class_name == "chrome_widgetwin_1";

                        FOUND_HWND.lock().unwrap().push((hwnd, is_terminal, is_vscode));
                    }
                }
            }
            1 // continue enumeration
        }
        winffi::EnumWindows(enum_callback as *const () as isize, 0);
    }

    // Log all found windows
    {
        let found = FOUND_HWND.lock().unwrap();
        log_relay(&format!("  EnumWindows found {} match(es)", found.len()));
        for (i, (h, is_term, is_vs)) in found.iter().enumerate() {
            // Get title for logging
            let title = unsafe {
                let len = winffi::GetWindowTextLengthW(*h);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    winffi::GetWindowTextW(*h, buf.as_mut_ptr(), len + 1);
                    String::from_utf16_lossy(&buf)
                } else {
                    String::from("<no title>")
                }
            };
            log_relay(&format!("    [{}] hwnd={} title=\"{}\" terminal={} vscode={}", i, h, title, is_term, is_vs));
        }
    }

    // Prefer terminal > non-VSCode > any match
    let hwnd = {
        let found = FOUND_HWND.lock().unwrap();
        found.iter()
            .find(|(_, is_terminal, _)| *is_terminal)
            .or_else(|| found.iter().find(|(_, _, is_vscode)| !is_vscode))
            .or_else(|| found.first())
            .map(|(h, _, _)| *h)
    };

    if let Some(hwnd) = hwnd {
        log_relay(&format!("  selected hwnd={}", hwnd));
        unsafe { paste_to_window(hwnd); }
        true
    } else {
        log_relay("  NO matching window found! Fallback to clipboard-only.");
        false
    }
}

// ---- Windows API FFI for paste_to_window ----

#[cfg(target_os = "windows")]
unsafe fn paste_to_window(target_hwnd: isize) {
    let prev = winffi::GetForegroundWindow();
    log_relay(&format!("  paste: prev_fg={}", prev));

    // Get thread IDs for input queue attachment
    let our_tid = winffi::GetCurrentThreadId();
    let target_tid = winffi::GetWindowThreadProcessId(target_hwnd, std::ptr::null_mut());
    log_relay(&format!("  paste: our_tid={} target_tid={}", our_tid, target_tid));

    // Attach input queues so keystrokes reach the target window, not the webview
    let attach_ok = winffi::AttachThreadInput(our_tid, target_tid, 1);
    log_relay(&format!("  paste: AttachThreadInput(1)={}", attach_ok));

    // Bring window to front (SW_RESTORE = 9, handles minimized windows)
    winffi::ShowWindow(target_hwnd, 9);
    winffi::SetForegroundWindow(target_hwnd);
    std::thread::sleep(std::time::Duration::from_millis(150));

    let actual_fg = winffi::GetForegroundWindow();
    log_relay(&format!("  paste: actual_fg={} (target={})", actual_fg, target_hwnd));
    if actual_fg != target_hwnd {
        log_relay("  WARNING: SetForegroundWindow did not change focus!");
    }

    // Ctrl+V via SendInput
    send_key_press(winffi::VK_CONTROL, 0);
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(winffi::VK_V, 0);
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(winffi::VK_V, winffi::KEYEVENTF_KEYUP);
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(winffi::VK_CONTROL, winffi::KEYEVENTF_KEYUP);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Enter via SendInput
    send_key_press(winffi::VK_RETURN, 0);
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(winffi::VK_RETURN, winffi::KEYEVENTF_KEYUP);
    log_relay("  paste: Ctrl+V + Enter sent");

    // Detach input queues
    winffi::AttachThreadInput(our_tid, target_tid, 0);

    // Restore previous foreground window
    std::thread::sleep(std::time::Duration::from_millis(150));
    if prev != 0 && prev != target_hwnd {
        winffi::SetForegroundWindow(prev);
        log_relay(&format!("  paste: restored prev_fg={}", prev));
    }
}

#[cfg(not(target_os = "windows"))]
fn relay_to_terminal(_msg: &str) -> bool { false }

#[tauri::command]
async fn approve_permission(app_state: State<'_, TauriAppState>) -> Result<(), String> {
    log_relay("approve_permission called");
    // Claude Code 权限菜单：输入 3 选择 "Always allow"
    let pasted = relay_to_terminal("3");
    // Clear Permission state so subsequent hooks can proceed
    clear_permission_state(&app_state).await;
    if !pasted {
        // Fallback: text already in clipboard, notify user
        let _ = app_state.tx.send(StateChangeEvent {
            animation: "waving".to_string(),
            bubble: "已复制「3」到剪贴板，请在终端 Ctrl+V".to_string(),
            overlay: None,
            input_type: None,
            options: None,
        });
    }
    Ok(())
}

#[tauri::command]
async fn deny_permission(app_state: State<'_, TauriAppState>) -> Result<(), String> {
    log_relay("deny_permission called");
    // Claude Code 权限菜单：输入 1 选择 "Deny"
    let pasted = relay_to_terminal("1");
    // Clear Permission state so subsequent hooks can proceed
    clear_permission_state(&app_state).await;
    if !pasted {
        // Fallback: text already in clipboard, notify user
        let _ = app_state.tx.send(StateChangeEvent {
            animation: "waving".to_string(),
            bubble: "已复制「1」到剪贴板，请在终端 Ctrl+V".to_string(),
            overlay: None,
            input_type: None,
            options: None,
        });
    }
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
