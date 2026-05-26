use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::state::{PetState, SharedState, StateChangeEvent};

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
}

pub fn create_router(
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    vis_tx: broadcast::Sender<super::VisibilityEvent>,
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
        .route("/api/hide", post(handle_hide))
        .route("/api/show", post(handle_show))
        .layer(cors)
        .with_state(AppState { state, tx, vis_tx })
}

#[derive(Clone)]
struct AppState {
    state: SharedState,
    tx: broadcast::Sender<StateChangeEvent>,
    vis_tx: broadcast::Sender<super::VisibilityEvent>,
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

    let _ = app.tx.send(StateChangeEvent { animation, bubble });
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
    Json(CurrentResponse { animation, bubble })
}

async fn handle_hook_thinking(
    State(app): State<AppState>,
) -> StatusCode {
    set_pet_state(&app, PetState::Chatting, None).await;
    StatusCode::OK
}

async fn handle_hook_working(
    State(app): State<AppState>,
    body: String,
) -> StatusCode {
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

    set_pet_state(&app, state.clone(), tool).await;

    // Cycle running variant for idle-type tools (running/left/right rotation)
    if matches!(state, PetState::Running) {
        let mut mgr = app.state.lock().await;
        mgr.set_running_variant();
    }

    StatusCode::OK
}

async fn handle_hook_done(
    State(app): State<AppState>,
) -> StatusCode {
    // Branch on tool type: Read/Glob/Grep/Agent → Review, others → Celebrating
    let done_state = {
        let mgr = app.state.lock().await;
        crate::state::StateManager::done_state_for_tool(mgr.current_tool())
    };
    set_pet_state(&app, done_state, None).await;
    StatusCode::OK
}

async fn handle_hook_idle(
    State(app): State<AppState>,
) -> StatusCode {
    set_pet_state(&app, PetState::Idle, None).await;
    StatusCode::OK
}

async fn handle_hook_permission(
    State(app): State<AppState>,
) -> StatusCode {
    {
        let mut mgr = app.state.lock().await;
        mgr.set_state(PetState::Permission, None);
    }
    let _ = app.tx.send(StateChangeEvent {
        animation: "waving".to_string(),
        bubble: "等待指示...".to_string(),
    });
    StatusCode::OK
}

async fn set_pet_state(app: &AppState, state: PetState, tool: Option<String>) {
    let mut mgr = app.state.lock().await;

    // Skip state changes while game_guard has forced the pet hidden (fullscreen)
    if mgr.is_force_hidden() {
        return;
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
        return;
    }

    mgr.set_state(state, tool.clone());
    let animation = mgr.current_animation_name();
    let bubble = mgr.current_state().bubble_text(tool.as_deref()).to_string();
    drop(mgr);

    let _ = app.tx.send(StateChangeEvent { animation, bubble });
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
