//! The API apps use. Plain JSON over HTTP:
//!
//! - `GET  /api`          → `{ "uuob": version, "device": name, "remote": bool }`
//!   (is UUOB here? `remote`: on another device, through the tunnel)
//! - `POST /api/connect`  `{ app, name, code? }` → `{ token }`. Without a code
//!   UUOB asks the user, and the request waits for the answer.
//! - `POST /api/call`     `{ method, params }` + bearer token → `{ result }`
//!   or `{ error: { code, message } }`. Methods: put, get, delete, list,
//!   containers, request_access, and for an app's extra containers (one per
//!   list, calendar, …): create_container, rename_container,
//!   delete_container, share. For files: files, delete_file (their bytes
//!   go over `/api/files`, see `files.rs`). On Android: android { package },
//!   which app this is, for its alerts and widget data.
//! - `GET  /api/events`   + bearer token → server-sent events
//!   `{ "container": id }` when a container the app can read changes
//!   (`"*"`: reload everything).

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::{Extension, Json};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event as Sse, KeepAlive};
use axum::response::{IntoResponse, Response, Sse as SseResponse};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use uuob_core::{Access, AppManifest, Error, Event, Node};

use crate::{Linked, bearer, client};

/// `remote`: the app is on another device and reaches this one through the tunnel.
pub(crate) async fn hello(State(node): State<Node>, linked: Option<Extension<Linked>>) -> Json<Value> {
    Json(json!({ "uuob": env!("CARGO_PKG_VERSION"), "device": node.status().device_name, "remote": linked.is_some() }))
}

pub(crate) fn error(e: Error) -> Response {
    let status = match e {
        Error::PermissionDenied(_) => StatusCode::FORBIDDEN,
        Error::NotFound(_) => StatusCode::NOT_FOUND,
        Error::Invalid(_) => StatusCode::BAD_REQUEST,
        Error::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let message = e.to_string();
    let message = message.split_once(": ").map_or(message.as_str(), |(_, m)| m);
    (status, Json(json!({ "error": { "code": e.code(), "message": message } }))).into_response()
}

pub(crate) fn unauthorized() -> Response {
    let body = json!({ "error": { "code": "unauthorized", "message": "not connected to UUOB" } });
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

/// JSON bodies only: browsers can't send those cross-site without our CORS
/// approval, so a web page can't pop up connection requests.
fn is_json(headers: &HeaderMap) -> bool {
    headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("application/json"))
}

#[derive(Deserialize)]
pub(crate) struct Connect {
    app: String,
    name: String,
    code: Option<String>,
}

pub(crate) async fn connect(State(node): State<Node>, headers: HeaderMap, body: String) -> Response {
    if !is_json(&headers) {
        return error(Error::Invalid("expected application/json".into()));
    }
    let Ok(c) = serde_json::from_str::<Connect>(&body) else {
        return error(Error::Invalid("expected { app, name, code? }".into()));
    };
    let manifest = AppManifest { id: c.app, name: c.name };
    match node.connect_app(&manifest, c.code.as_deref().filter(|c| !c.trim().is_empty())).await {
        Ok(token) => Json(json!({ "token": token })).into_response(),
        Err(e) => error(e),
    }
}

#[derive(Deserialize)]
pub(crate) struct Call {
    method: String,
    #[serde(default)]
    params: Value,
}

pub(crate) async fn call(State(node): State<Node>, linked: Option<Extension<Linked>>, headers: HeaderMap, body: String) -> Response {
    let Some(client) = client(&node, bearer(&headers), &linked) else { return unauthorized() };
    if !is_json(&headers) {
        return error(Error::Invalid("expected application/json".into()));
    }
    let Ok(c) = serde_json::from_str::<Call>(&body) else {
        return error(Error::Invalid("expected { method, params }".into()));
    };
    // `keep` is about the device the app runs on. An app on another device
    // has no UUOB there, and must not change what this device keeps.
    if linked.is_some() && c.method == "keep" {
        return Json(json!({ "result": null })).into_response();
    }
    // `android` { package }: which Android app this is, so this phone hands it
    // its alerts and widget data. Only for an app on this device.
    if c.method == "android" {
        let result = match (&linked, c.params["package"].as_str()) {
            (Some(_), _) => Err(Error::PermissionDenied("this app is not on this device".into())),
            (None, None) => Err(Error::Invalid("package is required".into())),
            (None, Some(package)) => node.set_android_package(bearer(&headers), package),
        };
        return match result {
            Ok(()) => Json(json!({ "result": null })).into_response(),
            Err(e) => error(e),
        };
    }
    match node.call(&client, &c.method, c.params).await {
        Ok(result) => Json(json!({ "result": result })).into_response(),
        Err(e) => error(e),
    }
}

pub(crate) async fn events(State(node): State<Node>, linked: Option<Extension<Linked>>, headers: HeaderMap) -> Response {
    let token = bearer(&headers).to_string();
    let Some(client) = client(&node, &token, &linked) else { return unauthorized() };
    let linked = linked.map(|l| l.0.0);
    let rx = node.subscribe();
    let token = (token, linked);
    let stream = futures_util::stream::unfold((node, client, token, rx, true), |(node, client, token, mut rx, first)| async move {
        let data = |c: &str| Ok::<_, Infallible>(Sse::default().data(json!({ "container": c }).to_string()));
        if first {
            return Some((data("*"), (node, client, token, rx, false)));
        }
        loop {
            match rx.recv().await {
                Ok(Event::ContainerChanged { container }) if node.allowed(&client, &container, Access::Read) => {
                    return Some((data(&container), (node, client, token, rx, false)));
                }
                // Grants or the connection itself may have changed.
                Ok(Event::NetworkChanged) => {
                    if node.authenticate_from(&token.0, token.1.as_deref()).as_ref() != Some(&client) {
                        return None;
                    }
                    return Some((data("*"), (node, client, token, rx, false)));
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => return Some((data("*"), (node, client, token, rx, false))),
                Err(RecvError::Closed) => return None,
            }
        }
    });
    sse(stream)
}

fn sse(s: impl Stream<Item = Result<Sse, Infallible>> + Send + 'static) -> Response {
    SseResponse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))).into_response()
}
