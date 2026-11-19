//! The UUOB app's Rust half, under its native screens: SwiftUI on the Mac
//! (`mac/`), Jetpack Compose on Android (`android/`) and GTK on Linux.
//!
//! It runs the node and the local server (for apps and AI clients), and it
//! keeps UUOB running after the window is closed: in the menu bar on a Mac,
//! in a background service on Android. The screens use a few functions,
//! which UniFFI makes into Swift and Kotlin (see `ffi.rs`):
//! - `start` UUOB, once per process;
//! - `call` a method, as the UUOB app itself;
//! - `next_event`, which waits for the next change.
//!
//! Every method is the node's (`Node::call`) except a few that belong to the
//! app rather than to the network (see [`local`]).

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod alerts;
pub mod ffi;
mod update;

uniffi::setup_scaffolding!();

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

use serde_json::{Value as Json, json};
use tokio::sync::broadcast::{self, error::RecvError};
use uuob_core::{Client, Config, Event, NetMode, Node};

/// UUOB's version, as its updates compare it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The async runtime everything runs on.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("uuob").build().expect("runtime"))
}

static NODE: tokio::sync::OnceCell<Node> = tokio::sync::OnceCell::const_new();
/// What changed, for the screen (see [`next_event`]).
static EVENTS: AsyncMutex<Option<broadcast::Receiver<Event>>> = AsyncMutex::const_new(None);
/// Why the local server couldn't start (e.g. the port is taken), if it didn't.
static SERVER_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Starts UUOB: the node and its local server. There is one per process,
/// started by whoever needs it first: the window or, on a phone that was just
/// turned on, the background service. Later calls return at once.
pub async fn start(data_dir: PathBuf) -> Result<(), String> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,iroh=warn,noq=warn".into()))
        .try_init();
    let data_dir = std::env::var_os("UUOB_DATA_DIR").map(Into::into).unwrap_or(data_dir);
    NODE.get_or_try_init(|| open(data_dir)).await.map(drop).map_err(|e| e.to_string())
}

async fn open(data_dir: PathBuf) -> uuob_core::Result<Node> {
    let node = Node::open(Config {
        data_dir,
        device_name: uuob_core::device_name(),
        platform: std::env::consts::OS.into(),
        net: NetMode::Internet,
        keystore: None,
    })
    .await?;
    *EVENTS.lock().await = Some(node.subscribe());
    watch_upcoming(node.clone());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        alerts::ring();
        autostart::refresh();
    }
    let n = node.clone();
    tokio::spawn(async move {
        let addr = ([127, 0, 0, 1], uuob_api::port()).into();
        if let Err(e) = uuob_api::serve(n, addr).await {
            eprintln!("local server not started: {e}");
            *SERVER_ERROR.lock().unwrap() = Some(format!("Apps can’t connect: port {} is in use ({e}).", uuob_api::port()));
        }
    });
    Ok(node)
}

/// Runs `method` as the UUOB app. An error comes with its code
/// (`permission_denied`, `invalid`, …) and a message for the user.
pub async fn call(method: &str, params: Json) -> Result<Json, (String, &'static str)> {
    if let Some(answer) = local(method, &params) {
        return answer.map_err(|e| (e, "internal"));
    }
    if matches!(method, "update_check" | "update_install") {
        // Downloads: off the async threads.
        let method = method.to_string();
        return tokio::task::spawn_blocking(move || match method.as_str() {
            "update_check" => update::check().map(|v| json!(v)),
            _ => update::install(&params),
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
        .map_err(|e| (e, "unavailable"));
    }
    let node = NODE.get().ok_or(("UUOB hasn’t started yet.".to_string(), "unavailable"))?;
    node.call(&Client::Shell, method, params).await.map_err(|e| (e.to_string(), e.code()))
}

/// The methods of the app itself, not of the network.
#[cfg_attr(any(target_os = "android", target_os = "ios"), allow(unused_variables))]
fn local(method: &str, params: &Json) -> Option<Result<Json, String>> {
    Some(match method {
        "version" => Ok(json!(VERSION)),
        "server_error" => Ok(json!(*SERVER_ERROR.lock().unwrap())),
        // The local MCP server's URL, or null on phones (AI clients run on computers).
        "mcp_url" => Ok(json!(
            cfg!(not(any(target_os = "android", target_os = "ios"))).then(|| format!("http://127.0.0.1:{}/mcp", uuob_api::port()))
        )),
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        "autostart" => Ok(json!(autostart::on())),
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        "set_autostart" => autostart::set(params["on"].as_bool().unwrap_or(false)).map(|_| Json::Null),
        _ => return None,
    })
}

/// Waits for the next change and returns it, e.g. `{"type": "device_joined",
/// "name": "…"}`. `{"type": "lagged"}` means changes were missed: reload all.
/// Meant for one caller, the screen's own loop.
pub async fn next_event() -> Json {
    let mut events = EVENTS.lock().await;
    let answer = match events.as_mut() {
        Some(rx) => match rx.recv().await {
            Ok(e) => return serde_json::to_value(e).unwrap_or_default(),
            Err(RecvError::Lagged(_)) => return json!({ "type": "lagged" }),
            Err(RecvError::Closed) => json!({ "type": "closed" }),
        },
        None => json!({ "type": "not_started" }),
    };
    // Nothing to wait for: don't let the screen's loop spin.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    answer
}

/// What is coming up in the user's apps (`Node::upcoming`) and the time
/// format to show it in, `{ hour12, items }` as JSON, kept current: for the
/// alerts (`alerts::ring` on computers, `Alerts.kt` on Android) and, on
/// Android, the apps' own widgets (`ToApps.kt`).
fn upcoming_now() -> &'static tokio::sync::watch::Sender<String> {
    static UPCOMING: OnceLock<tokio::sync::watch::Sender<String>> = OnceLock::new();
    UPCOMING.get_or_init(|| tokio::sync::watch::channel(String::new()).0)
}

fn watch_upcoming(node: Node) {
    tokio::spawn(async move {
        let mut events = node.subscribe();
        loop {
            let settings = node.call(&Client::Shell, "settings", Json::Null).await.unwrap_or_default();
            let hour12 = settings["hour12"].as_bool().unwrap_or(false);
            let json = json!({ "hour12": hour12, "items": node.upcoming() }).to_string();
            upcoming_now().send_if_modified(|v| *v != json && {
                *v = json.clone();
                true
            });
            // Wait for data to change, and for a burst of changes (a sync) to settle.
            loop {
                match events.recv().await {
                    Ok(Event::ContainerChanged { .. } | Event::NetworkChanged) | Err(RecvError::Lagged(_)) => break,
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            events = events.resubscribe();
        }
    });
}

/// Waits for what is coming up (`upcoming_now`) to differ from `known`, and
/// returns it. Android's `UuobService` follows it.
pub async fn next_upcoming(known: &str) -> String {
    let mut rx = upcoming_now().subscribe();
    loop {
        {
            let v = rx.borrow_and_update();
            if !v.is_empty() && *v != known {
                return v.clone();
            }
        }
        // The sender lives for good, so this only waits.
        let _ = rx.changed().await;
    }
}

/// "Start at login" on a computer: a LaunchAgent on a Mac, an autostart entry
/// on Linux, started with `--hidden` (the menu bar icon, no window). Android
/// keeps that choice itself (its `BootReceiver`).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod autostart {
    use std::path::PathBuf;

    fn file() -> Option<PathBuf> {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        if cfg!(target_os = "macos") {
            Some(home.join("Library/LaunchAgents/app.uuob.plist"))
        } else {
            let config = std::env::var_os("XDG_CONFIG_HOME").map(Into::into).unwrap_or_else(|| home.join(".config"));
            Some(config.join("autostart/uuob.desktop"))
        }
    }

    pub fn on() -> bool {
        file().is_some_and(|f| f.exists())
    }

    /// Points an existing entry at this copy of UUOB, which may have moved
    /// or been replaced by an update.
    pub fn refresh() {
        if on() {
            let _ = set(true);
        }
    }

    pub fn set(on: bool) -> Result<(), String> {
        let file = file().ok_or("this computer has no home folder")?;
        if !on {
            return match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let program = std::env::current_exe().map_err(|e| e.to_string())?;
        let program = program.to_string_lossy();
        let entry = if cfg!(target_os = "macos") {
            let program = program.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>app.uuob</string>
  <key>ProgramArguments</key><array><string>{program}</string><string>--hidden</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
"#
            )
        } else {
            format!("[Desktop Entry]\nType=Application\nName=UUOB\nExec=\"{program}\" --hidden\nX-GNOME-Autostart-enabled=true\n")
        };
        if std::fs::read_to_string(&file).is_ok_and(|now| now == entry) {
            return Ok(());
        }
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, entry).map_err(|e| e.to_string())
    }
}
