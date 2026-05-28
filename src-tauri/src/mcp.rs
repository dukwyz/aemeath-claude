use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::{PendingInput, SharedPendingInput, StateChangeEvent};

pub type McpState = Arc<Mutex<crate::state::StateManager>>;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

pub fn create_mcp_router(
    state: McpState,
    tx: broadcast::Sender<StateChangeEvent>,
    pending: SharedPendingInput,
) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp_request))
        .route("/sse", get(handle_sse))
        .with_state(McpAppState { state, tx, pending })
}

#[derive(Clone)]
struct McpAppState {
    state: McpState,
    tx: broadcast::Sender<StateChangeEvent>,
    pending: SharedPendingInput,
}

async fn handle_mcp_request(
    State(app): State<McpAppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let response = match req.method.as_str() {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "aemeath",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {},
                "resources": {}
            }
        }),

        "tools/list" => json!({
            "tools": [
                {
                    "name": "aemeath_show",
                    "description": "Show a custom bubble message on the Aemeath pet, optionally with a specific animation",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "msg": { "type": "string", "description": "Message to display" },
                            "animation": { "type": "string", "enum": ["idle", "waiting", "jumping", "waving", "running", "review", "failed", "celebrating"], "description": "Animation to play (optional, defaults to current)" }
                        },
                        "required": ["msg"]
                    }
                },
                {
                    "name": "aemeath_ask",
                    "description": "Ask the user a question through the pet UI",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "question": { "type": "string" },
                            "options": {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        },
                        "required": ["question"]
                    }
                },
                {
                    "name": "aemeath_play",
                    "description": "Force play a specific animation",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "state": { "type": "string", "enum": ["idle", "thinking", "running", "review", "failed", "waving", "jumping"] },
                            "duration_ms": { "type": "number" }
                        },
                        "required": ["state"]
                    }
                },
                {
                    "name": "aemeath_get_user_input",
                    "description": "Get user input through the pet UI. Supports confirm (yes/no), select (choose from options), and text (free-form input).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["confirm", "select", "text"], "description": "Type of input to request" },
                            "question": { "type": "string", "description": "Question or prompt to show the user" },
                            "options": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Options for select type"
                            }
                        },
                        "required": ["type", "question"]
                    }
                }
            ]
        }),

        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let tool_name = params["name"].as_str().unwrap_or("");
            let args = &params["arguments"];

            match tool_name {
                "aemeath_show" => {
                    let msg = args["msg"].as_str().unwrap_or("");
                    let animation = args["animation"].as_str().unwrap_or("waiting");
                    let _ = app.tx.send(StateChangeEvent {
                        animation: animation.to_string(),
                        bubble: msg.to_string(),
                        overlay: None,
                        input_type: None,
                        options: None,
                    });
                    json!({ "content": [{ "type": "text", "text": format!("Message shown: {}", msg) }] })
                }
                "aemeath_ask" => {
                    let question = args["question"].as_str().unwrap_or("");
                    let options: Vec<String> = args["options"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    // Create oneshot channel
                    let (tx, rx) = tokio::sync::oneshot::channel::<String>();

                    // Store pending input
                    {
                        let mut slot = app.pending.lock().await;
                        *slot = Some((
                            PendingInput {
                                input_type: "confirm".to_string(),
                                question: question.to_string(),
                                options: options.clone(),
                                created_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            },
                            tx,
                        ));
                    }

                    // Send overlay event so frontend shows interactive UI
                    let _ = app.tx.send(StateChangeEvent {
                        animation: "waving".into(),
                        bubble: String::new(),
                        overlay: Some("input".into()),
                        input_type: Some("confirm".into()),
                        options: Some(options),
                    });

                    // Block with 300s timeout
                    match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                        Ok(Ok(answer)) => {
                            json!({ "content": [{ "type": "text", "text": answer }] })
                        }
                        _ => {
                            // Timeout or channel closed — clear slot
                            let mut slot = app.pending.lock().await;
                            *slot = None;
                            json!({ "content": [{ "type": "text", "text": "User dismissed" }] })
                        }
                    }
                }
                "aemeath_get_user_input" => {
                    let input_type = args["type"].as_str().unwrap_or("confirm");
                    let question = args["question"].as_str().unwrap_or("");
                    let options: Vec<String> = args["options"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    // Create oneshot channel
                    let (tx, rx) = tokio::sync::oneshot::channel::<String>();

                    // Store pending input
                    {
                        let mut slot = app.pending.lock().await;
                        *slot = Some((
                            PendingInput {
                                input_type: input_type.to_string(),
                                question: question.to_string(),
                                options: options.clone(),
                                created_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            },
                            tx,
                        ));
                    }

                    // Send overlay event so frontend shows interactive UI
                    let _ = app.tx.send(StateChangeEvent {
                        animation: "waving".into(),
                        bubble: String::new(),
                        overlay: Some("input".into()),
                        input_type: Some(input_type.to_string()),
                        options: Some(options),
                    });

                    // Block with 300s timeout
                    match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                        Ok(Ok(answer)) => {
                            json!({ "content": [{ "type": "text", "text": answer }] })
                        }
                        _ => {
                            // Timeout or channel closed — clear slot
                            let mut slot = app.pending.lock().await;
                            *slot = None;
                            json!({ "content": [{ "type": "text", "text": "User dismissed" }] })
                        }
                    }
                }
                "aemeath_play" => {
                    let state_name = args["state"].as_str().unwrap_or("idle");
                    let _ = app.tx.send(StateChangeEvent {
                        animation: state_name.to_string(),
                        bubble: "".into(),
                        overlay: None,
                        input_type: None,
                        options: None,
                    });
                    json!({ "content": [{ "type": "text", "text": format!("Playing: {}", state_name) }] })
                }
                _ => {
                    return Json(JsonRpcResponse {
                        id: req.id,
                        result: None,
                        error: Some(json!({"code": -32601, "message": format!("Unknown tool: {}", tool_name)})),
                    });
                }
            }
        }

        "resources/list" => json!({
            "resources": [
                {
                    "uri": "aemeath://status",
                    "name": "Pet Status",
                    "description": "Current pet state and animation info"
                },
                {
                    "uri": "aemeath://history",
                    "name": "State History",
                    "description": "Recent state change records"
                }
            ]
        }),

        "resources/read" => {
            let params = req.params.unwrap_or_default();
            let uri = params["uri"].as_str().unwrap_or("");
            match uri {
                "aemeath://status" => {
                    let mgr = app.state.lock().await;
                    let current = mgr.current_state();
                    json!({
                        "contents": [{
                            "uri": "aemeath://status",
                            "text": format!("State: {:?}", current)
                        }]
                    })
                }
                "aemeath://history" => {
                    let mgr = app.state.lock().await;
                    let history = mgr.history();
                    json!({
                        "contents": [{
                            "uri": "aemeath://history",
                            "text": serde_json::to_string(history).unwrap_or_default()
                        }]
                    })
                }
                _ => {
                    return Json(JsonRpcResponse {
                        id: req.id,
                        result: None,
                        error: Some(json!({"code": -32602, "message": format!("Unknown resource: {}", uri)})),
                    });
                }
            }
        }

        _ => {
            return Json(JsonRpcResponse {
                id: req.id,
                result: None,
                error: Some(json!({"code": -32601, "message": format!("Unknown method: {}", req.method)})),
            });
        }
    };

    Json(JsonRpcResponse {
        id: req.id,
        result: Some(response),
        error: None,
    })
}

async fn handle_sse(
    State(app): State<McpAppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = app.tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .then(|result| async move {
            result.ok().map(|event| {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Ok::<_, Infallible>(Event::default().event("state-change").data(data))
            })
        })
        .filter_map(|opt| opt);
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
