//! What the screens use, made into Swift (Mac) and Kotlin (Android) by
//! UniFFI: `try await call("devices", "{}")` and `call("devices", "{}")`.
//! Methods and answers are JSON text, the same that apps and AI clients use,
//! so there is one list of methods and one place that checks permissions
//! (`Node::call`).

use serde_json::Value as Json;

/// A method that failed: `code` says what kind of failure it was
/// (`permission_denied`, `not_found`, `invalid`, `unavailable`, …),
/// `detail` says it to the user.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UuobError {
    #[error("{detail}")]
    Failed { code: String, detail: String },
}

/// Runs `fut` on UUOB's own runtime, whoever awaits it (Swift, Kotlin).
async fn on_runtime<T: Send + 'static>(fut: impl Future<Output = T> + Send + 'static) -> T {
    crate::runtime().spawn(fut).await.expect("UUOB task failed")
}

/// Starts UUOB with its data in `data_dir`. The window and the background
/// service may both call it; the second call returns at once.
#[uniffi::export]
pub async fn start(data_dir: String) -> Result<(), UuobError> {
    on_runtime(crate::start(data_dir.into())).await.map_err(|detail| UuobError::Failed { code: "unavailable".into(), detail })
}

/// Runs `method` with `params` (JSON, may be empty) as the UUOB app and
/// returns its answer as JSON.
#[uniffi::export]
pub async fn call(method: String, params: String) -> Result<String, UuobError> {
    let params = if params.trim().is_empty() { Json::Null } else { serde_json::from_str(&params).unwrap_or(Json::Null) };
    let answer = on_runtime(async move { crate::call(&method, params).await }).await;
    answer.map(|v| v.to_string()).map_err(|(detail, code)| UuobError::Failed { code: code.into(), detail })
}

/// Waits for the next change and returns it as JSON (see `crate::next_event`).
#[uniffi::export]
pub async fn next_event() -> String {
    on_runtime(crate::next_event()).await.to_string()
}

/// Waits for what is coming up in the user's apps to differ from `known`,
/// and returns it (Android's alerts and the apps' widgets).
#[uniffi::export]
pub async fn next_upcoming(known: String) -> String {
    on_runtime(async move { crate::next_upcoming(&known).await }).await
}

/// UUOB's version.
#[uniffi::export]
pub fn version() -> String {
    crate::VERSION.into()
}
