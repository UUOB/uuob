//! UUOB's local server, reachable only from this device (127.0.0.1).
//!
//! - `/api` for apps: connect (the user allows it, or types a code), then
//!   call the data methods and follow changes. Each app sees its own
//!   container plus whatever the user granted. `/api/files` carries file bytes.
//! - `/mcp` for AI clients created on UUOB's AI access screen.
//!
//! The one way in from outside the device: an app the user linked with a
//! code, on a device without UUOB, reaches the app routes through an
//! encrypted tunnel (see `uuob_core`'s `remote.rs` and [`serve_remote`]).

mod apps;
mod files;
mod mcp;

use std::net::SocketAddr;

use axum::extract::Request;
use axum::{Extension, Router};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuob_core::Node;

pub const DEFAULT_PORT: u16 = 7745;

/// The port for this user. Android profiles share one network stack, so
/// each profile gets its own port (7745 + profile number); otherwise apps in
/// one profile would reach UUOB in another. Apps compute the same number.
pub fn port() -> u16 {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            safe fn getuid() -> u32;
        }
        return DEFAULT_PORT + (getuid() / 100_000) as u16;
    }
    #[allow(unreachable_code)]
    DEFAULT_PORT
}

/// Serve until the task is dropped.
pub async fn serve(node: Node, addr: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("UUOB local server on http://{addr}");
    tokio::select! {
        r = axum::serve(listener, router(node.clone())) => r,
        _ = serve_remote(node) => Ok(()),
    }
}

/// What a connected app uses.
fn app_routes() -> Router<Node> {
    Router::new()
        .route("/api", get(apps::hello))
        .route("/api/call", post(apps::call))
        .route("/api/events", get(apps::events))
        .route("/api/files", post(files::upload))
        .route("/api/files/{id}", get(files::download))
}

fn guarded(routes: Router<Node>, node: Node) -> Router {
    // App windows (Tauri) may call us from their own origins; web pages may not.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|o: &HeaderValue, _| {
            let o = o.as_bytes();
            o == b"tauri://localhost" || o == b"http://tauri.localhost" || o == b"https://tauri.localhost"
                || o.starts_with(b"http://localhost:")
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::RANGE]);
    routes.layer(cors).layer(middleware::from_fn(local_host_only)).with_state(node)
}

pub fn router(node: Node) -> Router {
    let routes = app_routes()
        .route("/mcp", post(mcp::handle).get(|| async { StatusCode::METHOD_NOT_ALLOWED }))
        .route("/api/connect", post(apps::connect));
    guarded(routes, node)
}

/// The app on another device that a request came from, through the tunnel:
/// its endpoint id. Only that app's own token is accepted from it.
#[derive(Clone)]
pub(crate) struct Linked(pub String);

/// Who is asking: on this device any valid token counts; through the
/// tunnel only the token of the app at its other end.
pub(crate) fn client(node: &Node, token: &str, linked: &Option<Extension<Linked>>) -> Option<uuob_core::Client> {
    node.authenticate_from(token, linked.as_ref().map(|l| l.0.0.as_str()))
}

/// Serve apps on other devices that the user linked with a code. They get
/// the app routes only: not AI access, and not connecting (their code did
/// that). Runs until dropped.
pub async fn serve_remote(node: Node) {
    use tower::Service;
    let mut requests = node.remote_requests();
    let routes = guarded(app_routes(), node);
    while let Some(request) = requests.recv().await {
        let routes = routes.clone().layer(Extension(Linked(request.app)));
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |r: hyper::Request<hyper::body::Incoming>| routes.clone().call(r));
            let io = hyper_util::rt::TokioIo::new(request.io);
            let _ = hyper::server::conn::http1::Builder::new().serve_connection(io, service).await;
        });
    }
}

/// Refuse requests addressed to another host name (DNS rebinding).
async fn local_host_only(req: Request, next: Next) -> Response {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
    let name = host.rsplit_once(':').map_or(host, |(h, _)| h);
    if matches!(name, "127.0.0.1" | "localhost" | "[::1]") {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "local requests only").into_response()
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
}
