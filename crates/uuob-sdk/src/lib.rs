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

/// Where the app finds UUOB's API: the tunnel if the app is linked to a
/// UUOB on another device, otherwise the UUOB app on this one.
#[tauri::command]
async fn port<R: Runtime>(app: AppHandle<R>, remote: State<'_, Remote>) -> Result<u16, String> {
    match tunnel(&app, &remote).await {
        Ok(t) if t.linked().await => Ok(t.port()),
        _ => Ok(uuob_core::local_port()),
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

/// UUOB's answer to `nonce`, asked through Android so that only the real
/// UUOB can give it (see `SdkPlugin.kt` and `uuob_api::proof`). The SDK sends
/// its key to the port only if the port gives the same answer. `None`: UUOB
/// is not on this phone, or its port is not its own. Elsewhere, `None`.
#[tauri::command]
async fn prove<R: Runtime>(app: AppHandle<R>, nonce: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize)]
        struct Answer {
            proof: Option<String>,
        }
        let android = app.state::<Android<R>>();
        let answer: Answer = android.0.run_mobile_plugin_async("prove", serde_json::json!({ "nonce": nonce })).await.map_err(|e| e.to_string())?;
        return Ok(answer.proof);
    }
    #[allow(unreachable_code)]
    {
        let _ = (app, nonce);
        Ok(None)
    }
}

#[cfg(target_os = "android")]
struct Android<R: Runtime>(tauri::plugin::PluginHandle<R>);

/// The user disconnected this app in UUOB; its link is gone.
#[tauri::command]
async fn removed<R: Runtime>(app: AppHandle<R>, remote: State<'_, Remote>) -> Result<bool, String> {
    Ok(tunnel(&app, &remote).await.is_ok_and(|t| t.removed()))
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("uuob-sdk")
        .invoke_handler(tauri::generate_handler![port, link, removed, prove])
        .setup(|app, _api| {
            app.manage(Remote::default());
            #[cfg(target_os = "android")]
            app.manage(Android(_api.register_android_plugin("app.uuob.sdk", "SdkPlugin")?));
            Ok(())
        })
        .build()
}
