use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::state::{PetState, SharedPendingInput, SharedState, StateChangeEvent};

#[derive(Debug, Deserialize)]
pub struct StateRequest {
    pub s: String,
    #[serde(default)]
    pub tool: Option<String>,
}


#[derive(Debug, Serialize)]
pub struct CurrentResponse {
    pub animation: String,
    pub bubble: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

pub fn create_router(
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    vis_tx: broadcast::Sender<super::VisibilityEvent>,
    pending: SharedPendingInput,
    claude_hwnd: std::sync::Arc<std::sync::Mutex<isize>>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/state", post(handle_state))
        .route("/api/heartbeat", get(handle_heartbeat))
        .route("/api/current", get(handle_current))
        .route("/api/hook/thinking", post(handle_hook_thinking))
        .route("/api/hook/working", post(handle_hook_working))
        .route("/api/hook/done", post(handle_hook_done))
        .route("/api/hook/idle", post(handle_hook_idle))
        .route("/api/hook/permission", post(handle_hook_permission))
        .route("/api/hook/notification", post(handle_hook_notification))
        .route("/api/hook/subagent-done", post(handle_hook_subagent_done))
        .route("/api/hide", post(handle_hide))
        .route("/api/show", post(handle_show))
        .route("/api/user/pending", get(handle_user_pending))
        .route("/api/user/input", post(handle_user_input))
        .route("/api/user/message", post(handle_user_message))
        .route("/api/user/message/pending", get(handle_user_message_pending))
        .layer(cors)
        .with_state(AppState { state, tx, vis_tx, pending, claude_hwnd })
}

#[derive(Clone)]
struct AppState {
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    vis_tx: broadcast::Sender<super::VisibilityEvent>,
    pending: SharedPendingInput,
    claude_hwnd: std::sync::Arc<std::sync::Mutex<isize>>,
}

async fn handle_state(
    State(app): State<AppState>,
    Json(body): Json<StateRequest>,
) -> StatusCode {
    let pet_state = PetState::from_hook(&body.s, body.tool.as_deref());
    let tool = body.tool.clone();

    {
        let mut mgr = app.state.lock().await;
        mgr.set_state(pet_state, tool);
    }

    // Read back with variant awareness
    let mgr = app.state.lock().await;
    let animation = mgr.current_animation_name();
    let bubble = mgr.current_state().bubble_text(mgr.current_tool()).to_string();
    drop(mgr);

    let _ = app.tx.send(StateChangeEvent {
        animation,
        bubble,
        overlay: None,
        input_type: None,
        options: None,
    });
    StatusCode::OK
}

async fn handle_heartbeat() -> StatusCode {
    StatusCode::OK
}

async fn handle_current(
    State(app): State<AppState>,
) -> Json<CurrentResponse> {
    let mgr = app.state.lock().await;
    let tool = mgr.current_tool();
    let animation = mgr.current_animation_name();
    let bubble = mgr.current_state().bubble_text(tool).to_string();
    drop(mgr);

    // Include pending input state so frontend can drive overlay
    let pending = app.pending.lock().await;
    let (overlay, input_type, options, question) = match pending.as_ref() {
        Some((p, _)) => (
            Some("input".to_string()),
            Some(p.input_type.clone()),
            Some(p.options.clone()),
            Some(p.question.clone()),
        ),
        None => (None, None, None, None),
    };

    Json(CurrentResponse { animation, bubble, overlay, input_type, options, question })
}

async fn handle_hook_thinking(
    State(app): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    set_pet_state(&app, PetState::Chatting, None).await
        .map_err(|e| { eprintln!("[hook/thinking] {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(StatusCode::OK)
}

async fn handle_hook_working(
    State(app): State<AppState>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let tool = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("tool_name").and_then(|t| t.as_str().map(String::from)));

    // Map specific tools to specific animations
    let state = match tool.as_deref() {
        Some("WebFetch") => PetState::Fetching,
        Some("WebSearch") => PetState::Searching,
        Some("Write") | Some("Edit") => PetState::Building,
        Some("Agent") | Some("TaskCreate") | Some("TaskUpdate") => PetState::Analyzing,
        _ => PetState::Running,
    };

    set_pet_state(&app, state.clone(), tool).await
        .map_err(|e| { eprintln!("[hook/working] {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    // Cycle running variant for idle-type tools (running/left/right rotation)
    if matches!(state, PetState::Running) {
        let mut mgr = app.state.lock().await;
        mgr.set_running_variant();
    }

    Ok(StatusCode::OK)
}

async fn handle_hook_done(
    State(app): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    // Branch on tool type: Read/Glob/Grep/Agent → Review, others → Celebrating
    let done_state = {
        let mgr = app.state.lock().await;
        crate::state::StateManager::done_state_for_tool(mgr.current_tool())
    };
    set_pet_state(&app, done_state, None).await
        .map_err(|e| { eprintln!("[hook/done] {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(StatusCode::OK)
}

async fn handle_hook_idle(
    State(app): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    set_pet_state(&app, PetState::Idle, None).await
        .map_err(|e| { eprintln!("[hook/idle] {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(StatusCode::OK)
}

async fn handle_hook_permission(
    State(app): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    {
        let mut mgr = app.state.lock().await;
        // Skip permission while pet is hidden (fullscreen)
        if mgr.is_force_hidden() {
            return Ok(StatusCode::OK);
        }
        // Intentionally bypasses set_pet_state() to avoid the Permission
        // protection guard — Permission must be enterable from any state.
        mgr.set_state(PetState::Permission, None);
    }
    let _ = app.tx.send(StateChangeEvent {
        animation: "waving".to_string(),
        bubble: "等待指示...".to_string(),
        overlay: None,
        input_type: None,
        options: None,
    });
    Ok(StatusCode::OK)
}

async fn handle_hook_notification(
    State(app): State<AppState>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let msg = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str().map(String::from)))
        .unwrap_or_else(|| "Notification".to_string());

    {
        let mut mgr = app.state.lock().await;
        if mgr.is_force_hidden() {
            return Ok(StatusCode::OK);
        }
        mgr.set_state(PetState::Chatting, None);
    }

    let _ = app.tx.send(StateChangeEvent {
        animation: "waiting".to_string(),
        bubble: msg,
        overlay: None,
        input_type: None,
        options: None,
    });
    Ok(StatusCode::OK)
}

async fn handle_hook_subagent_done(
    State(app): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    set_pet_state(&app, PetState::Celebrating, None).await
        .map_err(|e| { eprintln!("[hook/subagent-done] {}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    Ok(StatusCode::OK)
}

async fn set_pet_state(app: &AppState, state: PetState, tool: Option<String>) -> Result<(), String> {
    let mut mgr = app.state.lock().await;

    // Skip state changes while game_guard has forced the pet hidden (fullscreen)
    if mgr.is_force_hidden() {
        return Ok(());
    }

    // Protect Permission state: don't let other hooks override it while waiting for user input.
    // Only a new permission hook or the permission resolution can change this state.
    if matches!(mgr.current_state(), PetState::Permission)
        && !matches!(state, PetState::Permission)
    {
        return Ok(());
    }

    // Protect tool bubble minimum display time (all active states)
    let is_active = matches!(mgr.current_state(),
        PetState::Running | PetState::Chatting | PetState::Fetching
        | PetState::Searching | PetState::Analyzing | PetState::Building
    );
    if !matches!(state, PetState::Running | PetState::Chatting | PetState::Fetching
        | PetState::Searching | PetState::Analyzing | PetState::Building)
        && is_active
        && mgr.should_keep_running(800)
    {
        return Ok(());
    }

    mgr.set_state(state, tool.clone());
    let animation = mgr.current_animation_name();
    let bubble = mgr.current_state().bubble_text(tool.as_deref()).to_string();
    drop(mgr);

    app.tx.send(StateChangeEvent {
        animation,
        bubble,
        overlay: None,
        input_type: None,
        options: None,
    }).map_err(|e| format!("broadcast send failed: {}", e))?;

    Ok(())
}

async fn handle_hide(
    State(app): State<AppState>,
) -> StatusCode {
    {
        let mut mgr = app.state.lock().await;
        mgr.force_hide();
    }
    let _ = app.vis_tx.send(super::VisibilityEvent { visible: false });
    StatusCode::OK
}

async fn handle_show(
    State(app): State<AppState>,
) -> StatusCode {
    {
        let mut mgr = app.state.lock().await;
        mgr.release_hide();
    }
    let _ = app.vis_tx.send(super::VisibilityEvent { visible: true });
    StatusCode::OK
}

// ---- Pending Input endpoints (oneshot channel) ----

#[derive(Serialize)]
pub struct PendingResponse {
    pub has_pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

async fn handle_user_pending(
    State(app): State<AppState>,
) -> Json<PendingResponse> {
    let slot = app.pending.lock().await;
    match slot.as_ref() {
        Some((pending, _)) => Json(PendingResponse {
            has_pending: true,
            input_type: Some(pending.input_type.clone()),
            question: Some(pending.question.clone()),
            options: Some(pending.options.clone()),
        }),
        None => Json(PendingResponse {
            has_pending: false,
            input_type: None,
            question: None,
            options: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct UserInputRequest {
    pub answer: String,
}

#[derive(Debug, Deserialize)]
pub struct UserMessageRequest {
    pub value: String,
}

async fn handle_user_input(
    State(app): State<AppState>,
    Json(body): Json<UserInputRequest>,
) -> StatusCode {
    let sender = {
        let mut slot = app.pending.lock().await;
        slot.take().map(|(_, sender)| sender)
    };
    match sender {
        Some(tx) => {
            let _ = tx.send(body.answer);
            StatusCode::OK
        }
        None => StatusCode::CONFLICT, // 409: no pending input to answer
    }
}

// ---- Message Sync endpoints (pet → Claude Code terminal) ----

/// POST /api/user/message — receive message from pet UI, relay to Claude Code terminal
async fn handle_user_message(
    State(app): State<AppState>,
    Json(body): Json<UserMessageRequest>,
) -> StatusCode {
    let msg = body.value.clone();

    // Store message for MCP resource consumption
    {
        let mut mgr = app.state.lock().await;
        mgr.push_message(msg.clone());
    }

    // Relay the message to Claude Code terminal as keystrokes
    let hwnd = *app.claude_hwnd.lock().unwrap();
    relay_to_terminal(&msg, hwnd);

    StatusCode::OK
}

/// GET /api/user/message/pending — return and clear pending user messages
async fn handle_user_message_pending(
    State(app): State<AppState>,
) -> Json<serde_json::Value> {
    let mut mgr = app.state.lock().await;
    let msgs = mgr.drain_messages();
    Json(serde_json::json!({
        "messages": msgs,
        "count": msgs.len(),
    }))
}

/// Copy message to clipboard, then paste into the Claude Code terminal window.
fn relay_to_terminal(msg: &str, bound_hwnd: isize) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 1. Copy message to clipboard via PowerShell
    let escaped = msg.replace('\'', "''");
    let set_clip = format!("Set-Clipboard -Value '{}'", escaped);
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &set_clip])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    // 2. Find Claude Code window and paste
    std::thread::spawn(move || {
        unsafe { paste_to_window(bound_hwnd); }
    });
}

// ---- Windows API FFI for message relay ----

static FOUND_HWND: std::sync::Mutex<isize> = std::sync::Mutex::new(0);

const SEARCH_TERMS: &[&str] = &["claude", "obsidian"];

unsafe extern "system" fn enum_callback(hwnd: isize, _lparam: isize) -> i32 {
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
    if len > 0 {
        let title = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
        for term in SEARCH_TERMS {
            if title.contains(term) {
                *FOUND_HWND.lock().unwrap() = hwnd;
                return 0; // stop
            }
        }
    }
    1 // continue
}

unsafe fn send_key_press(key: u8, flags: u32) {
    // Win32 INPUT structure (x64): 40 bytes
    let mut input = [0u8; 40];
    // type = INPUT_KEYBOARD (1)
    input[0..4].copy_from_slice(&1u32.to_le_bytes());
    // wVk at offset 8
    input[8..10].copy_from_slice(&key.to_le_bytes());
    // dwFlags at offset 12
    input[12..16].copy_from_slice(&flags.to_le_bytes());
    SendInput(1, input.as_ptr(), 40);
}

unsafe fn paste_to_window(bound_hwnd: isize) {
    // 1. Search for the current topmost "claude" window
    *FOUND_HWND.lock().unwrap() = 0;
    EnumWindows(Some(enum_callback), 0);
    let mut hwnd = *FOUND_HWND.lock().unwrap();

    // 2. Fall back to bound HWND if search found nothing (e.g. terminal minimized)
    if hwnd == 0 && bound_hwnd != 0 && IsWindow(bound_hwnd) != 0 {
        hwnd = bound_hwnd;
    }

    if hwnd == 0 {
        return;
    }

    let prev = GetForegroundWindow();

    // Get thread IDs for input queue attachment
    let our_tid = GetCurrentThreadId();
    let target_tid = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());

    // Attach input queues so keystrokes reach the target window, not the webview
    AttachThreadInput(our_tid, target_tid, 1);

    ShowWindow(hwnd, 9); // SW_RESTORE
    SetForegroundWindow(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(150));

    // Ctrl+V via SendInput
    send_key_press(0x11, 0);  // VK_CONTROL down
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(0x56, 0);  // VK_V down
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(0x56, 2);  // VK_V up
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(0x11, 2);  // VK_CONTROL up
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Enter via SendInput
    send_key_press(0x0D, 0);  // VK_RETURN down
    std::thread::sleep(std::time::Duration::from_millis(10));
    send_key_press(0x0D, 2);  // VK_RETURN up

    // Detach input queues
    AttachThreadInput(our_tid, target_tid, 0);

    // Restore previous focus
    std::thread::sleep(std::time::Duration::from_millis(150));
    if prev != 0 {
        SetForegroundWindow(prev);
    }
}

#[allow(clashing_extern_declarations)]
extern "system" {
    fn EnumWindows(cb: Option<unsafe extern "system" fn(isize, isize) -> i32>, lp: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn AttachThreadInput(idAttach: u32, idAttachTo: u32, fAttach: i32) -> i32;
    fn GetWindowThreadProcessId(hWnd: isize, lpdwProcessId: *mut u32) -> u32;
    fn GetCurrentThreadId() -> u32;
    fn SendInput(nInputs: u32, pInputs: *const u8, cbSize: i32) -> u32;
}
