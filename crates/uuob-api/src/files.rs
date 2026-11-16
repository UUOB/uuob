//! File bytes for apps (see `uuob_core`'s `files.rs`):
//!
//! - `POST /api/files?container=<id>`: the body is the file, `Content-Type`
//!   its type → `{ id, size, type, added, here }`.
//! - `GET  /api/files/<id>?container=<id>` → the bytes. `Range` requests
//!   work, so `<video>` can seek. The token may also be given as `&key=`,
//!   because `<img>` and `<video>` cannot send headers.
//!
//! Listing and deleting go through `/api/call` (`files`, `delete_file`).

use axum::body::{Body, Bytes};
use axum::{Extension, Json};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::{StreamExt, stream};
use serde::Deserialize;
use uuob_core::{Client, Error, Node};

use crate::apps::{error, unauthorized};
use crate::{Linked, bearer};

#[derive(Deserialize)]
pub(crate) struct Where {
    container: Option<String>,
    key: Option<String>,
}

/// Who is asking, and for which container (an app's own by default).
fn caller(node: &Node, linked: &Option<Extension<Linked>>, headers: &HeaderMap, q: &Where) -> Result<(Client, String), Response> {
    let token = q.key.as_deref().unwrap_or_else(|| bearer(headers));
    let client = crate::client(node, token, linked).ok_or_else(unauthorized)?;
    let container = match (&q.container, &client) {
        (Some(c), _) => c.clone(),
        (None, Client::App(a)) => a.clone(),
        (None, _) => return Err(error(Error::Invalid("container is required".into()))),
    };
    Ok((client, container))
}

pub(crate) async fn upload(
    State(node): State<Node>,
    linked: Option<Extension<Linked>>,
    headers: HeaderMap,
    Query(q): Query<Where>,
    body: Body,
) -> Response {
    let (client, container) = match caller(&node, &linked, &headers, &q) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let content_type = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let mut file = match node.new_file(&client, &container, content_type).await {
        Ok(f) => f,
        Err(e) => return error(e),
    };
    let mut chunks = body.into_data_stream();
    while let Some(chunk) = chunks.next().await {
        let Ok(chunk) = chunk else { return error(Error::Invalid("the upload was cut off".into())) };
        if let Err(e) = file.write(&chunk).await {
            return error(e);
        }
    }
    match file.finish().await {
        Ok(info) => Json(info).into_response(),
        Err(e) => error(e),
    }
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` (the last n bytes).
enum Range {
    From(u64, Option<u64>),
    Last(u64),
}

fn range(headers: &HeaderMap) -> Option<Range> {
    let v = headers.get(header::RANGE)?.to_str().ok()?.strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    match (a.trim(), b.trim()) {
        ("", n) => Some(Range::Last(n.parse().ok()?)),
        (a, "") => Some(Range::From(a.parse().ok()?, None)),
        (a, b) => Some(Range::From(a.parse().ok()?, Some(b.parse().ok()?))),
    }
}

pub(crate) async fn download(
    State(node): State<Node>,
    linked: Option<Extension<Linked>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Where>,
) -> Response {
    let (client, container) = match caller(&node, &linked, &headers, &q) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let wanted = range(&headers);
    let (start, end) = match wanted {
        None => (0, None),
        Some(Range::From(a, b)) => (a, b.map(|b| b.saturating_add(1))),
        Some(Range::Last(n)) => match node.read_file(&client, &container, &id, 0, Some(0)).await {
            Ok((info, _)) => (info.size.saturating_sub(n), None),
            Err(e) => return error(e),
        },
    };
    let (info, reader) = match node.read_file(&client, &container, &id, start, end).await {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    let end = end.unwrap_or(info.size).min(info.size);
    if wanted.is_some() && start >= end {
        return (StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{}", info.size))])
            .into_response();
    }
    let bytes = stream::unfold(reader, |mut r| async move {
        match r.next().await {
            Ok(Some(b)) => Some((Ok(Bytes::from(b)), r)),
            Ok(None) => None,
            Err(e) => Some((Err(std::io::Error::other(e.to_string())), r)),
        }
    });
    let mut res = Response::builder()
        .status(if wanted.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
        .header(header::CONTENT_TYPE, info.content_type)
        .header(header::CONTENT_LENGTH, end - start.min(end))
        .header(header::ACCEPT_RANGES, "bytes")
        // A file never changes under its id.
        .header(header::CACHE_CONTROL, "private, max-age=31536000, immutable")
        // Whatever the file claims to be, it is never run as a page of ours.
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, "sandbox");
    if wanted.is_some() {
        res = res.header(header::CONTENT_RANGE, format!("bytes {start}-{}/{}", end - 1, info.size));
    }
    res.body(Body::from_stream(bytes)).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
