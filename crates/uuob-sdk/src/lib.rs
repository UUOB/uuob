//! The native half of the UUOB SDK (`ui/uuob.js`), for an app's shell:
//! `.plugin(tauri_plugin_uuob_sdk::init())`.
//!
//! An app has no backend of its own. It talks to UUOB's local API, which is
//! either the UUOB app on this device, or, when the device has none, a
//! tunnel to the user's UUOB on another device (after the user typed a code
//! from UUOB → Apps into the app; see `uuob_core`'s `remote.rs`). Both look
//! the same to the app: a port on 127.0.0.1.

use serde::Serialize;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Manager, Runtime, State};
use tokio::sync::OnceCell;
use uuob_core::{AppManifest, NetMode, Tunnel};

#[derive(Default)]
struct Remote(OnceCell<Tunnel>);

async fn tunnel<R: Runtime>(app: &AppHandle<R>, remote: &Remote) -> Result<Tunnel, String> {
    let open = || async {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        Tunnel::open(dir, NetMode::Internet).await.map_err(|e| e.to_string())
    };
    remote.0.get_or_try_init(open).await.cloned()
}

/// The port of the UUOB app on this device, for this user: 7745, plus the
/// profile number on Android (profiles share one network stack; this keeps
/// each profile's apps with that profile's UUOB). Must match `uuob_api::port()`.
fn local_port() -> u16 {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            safe fn getuid() -> u32;
        }
        return 7745 + (getuid() / 100_000) as u16;
    }
    #[allow(unreachable_code)]
    7745
}

/// Where the app finds UUOB's API: the tunnel if the app is linked to a
/// UUOB on another device, otherwise the UUOB app on this one.
#[tauri::command]
async fn port<R: Runtime>(app: AppHandle<R>, remote: State<'_, Remote>) -> Result<u16, String> {
    match tunnel(&app, &remote).await {
        Ok(t) if t.linked().await => Ok(t.port()),
        _ => Ok(local_port()),
    }
}

#[derive(Serialize)]
struct Linked {
    token: String,
    port: u16,
}

/// Link to the UUOB that showed `code` (UUOB → Apps → Connect with a code,
/// on another device). The app gets its token and the port to use from now on.
#[tauri::command]
async fn link<R: Runtime>(app: AppHandle<R>, remote: State<'_, Remote>, code: String, id: String, name: String) -> Result<Linked, String> {
    let t = tunnel(&app, &remote).await?;
    let token = t.link(&code, &AppManifest { id, name }).await.map_err(|e| {
        let text = e.to_string();
        text.split_once(": ").map_or(text.clone(), |(_, m)| m.to_string())
    })?;
    Ok(Linked { token, port: t.port() })
}

/// The user disconnected this app in UUOB; its link is gone.
#[tauri::command]
async fn removed<R: Runtime>(app: AppHandle<R>, remote: State<'_, Remote>) -> Result<bool, String> {
    Ok(tunnel(&app, &remote).await.is_ok_and(|t| t.removed()))
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("uuob-sdk")
        .invoke_handler(tauri::generate_handler![port, link, removed])
        .setup(|app, _api| {
            app.manage(Remote::default());
            Ok(())
        })
        .build()
}
