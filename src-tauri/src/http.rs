use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
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
        .layer(cors)
        .with_state(AppState { state, tx, vis_tx, pending })
}

#[derive(Clone)]
struct AppState {
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    vis_tx: broadcast::Sender<super::VisibilityEvent>,
    pending: SharedPendingInput,
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
