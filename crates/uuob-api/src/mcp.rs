//! MCP (Streamable HTTP, JSON responses) for AI access to UUOB.
//!
//! Every request needs `Authorization: Bearer
//! <token>` from a client created in the UUOB app; the client sees only the
//! containers it was granted, with the access it was granted.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value as Json, json};
use uuob_core::{Client, Node};

use crate::bearer;

const PROTOCOL_VERSION: &str = "2025-06-18";

fn tools() -> Json {
    let rec = |extra: Json| {
        let mut props = json!({
            "container": {"type": "string", "description": "Container id, e.g. \"tasks\". An app can have several (one per list or calendar). See list_containers."},
            "collection": {"type": "string", "description": "Collection inside the container, e.g. \"tasks\" or \"events\"."}
        });
        let mut required = vec!["container", "collection"];
        if let Json::Object(m) = extra {
            for (k, v) in m {
                if v.get("required").and_then(Json::as_bool) == Some(true) {
                    required.push(Box::leak(k.clone().into_boxed_str()));
                }
                let mut v = v;
                v.as_object_mut().unwrap().remove("required");
                props[&k] = v;
            }
        }
        json!({"type": "object", "properties": props, "required": required})
    };
    json!([
        {
            "name": "list_containers",
            "description": "List the data containers you may access, with their app, name, collections and record counts. An app can have several containers (e.g. one per list or calendar); some may be shared by another person and view-only (can_write).",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "list_records",
            "description": "List all records of a collection. Each record is a JSON object with an \"id\".",
            "inputSchema": rec(json!({}))
        },
        {
            "name": "get_record",
            "description": "Get one record by id.",
            "inputSchema": rec(json!({"id": {"type": "string", "required": true}}))
        },
        {
            "name": "put_record",
            "description": "Create or replace a record (needs write access). Omit id to create a new record. Follow the field names used by existing records.",
            "inputSchema": rec(json!({
                "id": {"type": "string"},
                "value": {"type": "object", "description": "The full record (fields not included are removed).", "required": true}
            }))
        },
        {
            "name": "delete_record",
            "description": "Delete a record by id (needs write access).",
            "inputSchema": rec(json!({"id": {"type": "string", "required": true}}))
        }
    ])
}

pub(crate) async fn handle(State(node): State<Node>, headers: HeaderMap, body: String) -> Response {
    let Some(client) = node.authenticate(bearer(&headers)) else {
        return (StatusCode::UNAUTHORIZED, "missing or invalid token; create one in UUOB → AI access").into_response();
    };
    let Ok(req) = serde_json::from_str::<Json>(&body) else {
        return rpc_error(Json::Null, -32700, "parse error");
    };
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(Json::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Json::Null);

    // Notifications (no id) get 202 and no body.
    let Some(id) = id else { return StatusCode::ACCEPTED.into_response() };

    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "uuob", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Personal data from the user's own UUOB network (calendar, tasks, ...). Call list_containers first."
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => call_tool(&node, &client, &params).await,
        _ => return rpc_error(id, -32601, "method not found"),
    };
    axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response()
}

async fn call_tool(node: &Node, client: &Client, params: &Json) -> Json {
    let name = params.get("name").and_then(Json::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let method = match name {
        "list_containers" => "containers",
        "list_records" => "list",
        "get_record" => "get",
        "put_record" => "put",
        "delete_record" => "delete",
        _ => return tool_result(format!("unknown tool {name}"), true),
    };
    match node.call(client, method, args).await {
        Ok(v) => tool_result(serde_json::to_string_pretty(&v).unwrap_or_default(), false),
        Err(e) => tool_result(format!("{}: {e}", e.code()), true),
    }
}

fn tool_result(text: String, is_error: bool) -> Json {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

fn rpc_error(id: Json, code: i64, message: &str) -> Response {
    axum::Json(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})).into_response()
}
