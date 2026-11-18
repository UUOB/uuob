//! In-app updates for one app. The app says where its updates are published
//! in its own `tauri.conf.json` (`plugins.updater`: `endpoints` and `pubkey`),
//! so every author publishes and signs their own.
//!
//! The web side has two commands: `check` (the newer version, if there is one)
//! and `install`. On computers they use Tauri's updater, which replaces the
//! app and restarts it. Tauri's updater doesn't cover Android, so there the
//! same feed and the same signatures are used, and the APK is handed to the
//! system installer.
//!
//! The feed is Tauri's: `{ "version": "0.1.2", "platforms": { "<target>":
//! { "url": …, "signature": … } } }`, with `android-aarch64` next to
//! `darwin-aarch64` and the others.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{AppHandle, Runtime};

#[cfg(any(target_os = "android", test))]
mod feed;

#[cfg(desktop)]
mod platform {
    use tauri::{AppHandle, Runtime};
    use tauri_plugin_updater::UpdaterExt;

    pub async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
        let update = app.updater().map_err(text)?.check().await.map_err(text)?;
        Ok(update.map(|u| u.version))
    }

    pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
        let Some(update) = app.updater().map_err(text)?.check().await.map_err(text)? else { return Ok(()) };
        update.download_and_install(|_, _| {}, || {}).await.map_err(text)?;
        app.restart()
    }

    fn text(e: impl std::fmt::Display) -> String {
        e.to_string()
    }
}

#[cfg(target_os = "android")]
mod platform {
    use crate::feed::{self, File};
    use serde::Deserialize;
    use serde_json::json;
    use tauri::plugin::{PluginApi, PluginHandle, mobile::PluginInvokeError};
    use tauri::{AppHandle, Manager, Runtime};

    struct Android<R: Runtime>(PluginHandle<R>);

    #[derive(Deserialize)]
    struct Saved {
        path: String,
    }

    pub fn setup<R: Runtime>(app: &AppHandle<R>, api: PluginApi<R, ()>) -> Result<(), Box<dyn std::error::Error>> {
        app.manage(Android(api.register_android_plugin("app.uuob.update", "UpdatePlugin")?));
        Ok(())
    }

    /// Where this app's updates are and the key they are signed with.
    fn source<R: Runtime>(app: &AppHandle<R>) -> Result<(String, String), String> {
        let config = app.config().plugins.0.get("updater").ok_or("this app has no update source")?;
        let endpoint = config["endpoints"][0].as_str().ok_or("this app has no update source")?;
        let pubkey = config["pubkey"].as_str().ok_or("this app has no update key")?;
        Ok((endpoint.into(), pubkey.into()))
    }

    /// Saves `url` in the app's cache and returns where, with what was saved.
    async fn download<R: Runtime>(app: &AppHandle<R>, url: &str, name: &str) -> Result<(String, Vec<u8>), String> {
        let saved: Saved = run(app, "download", json!({ "url": url, "name": name })).await?;
        let data = std::fs::read(&saved.path).map_err(|e| e.to_string())?;
        Ok((saved.path, data))
    }

    async fn run<R: Runtime, T: serde::de::DeserializeOwned>(app: &AppHandle<R>, command: &str, args: serde_json::Value) -> Result<T, String> {
        app.state::<Android<R>>().0.run_mobile_plugin_async(command, args).await.map_err(|e| match e {
            PluginInvokeError::InvokeRejected(r) => r.message.unwrap_or_else(|| "The update failed".into()),
            e => e.to_string(),
        })
    }

    /// The newer version and its file for this phone, if one is published.
    async fn newer<R: Runtime>(app: &AppHandle<R>) -> Result<Option<(String, File, String)>, String> {
        let (endpoint, pubkey) = source(app)?;
        let (_, published) = download(app, &endpoint, "update.json").await?;
        let target = format!("android-{}", std::env::consts::ARCH);
        let found = feed::newer(&published, &app.package_info().version, &target)?;
        Ok(found.map(|(version, file)| (version, file, pubkey)))
    }

    pub async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
        Ok(newer(app).await?.map(|(version, ..)| version))
    }

    pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
        let Some((_, file, pubkey)) = newer(app).await? else { return Ok(()) };
        let (path, apk) = download(app, &file.url, "update.apk").await?;
        feed::verify(&apk, &file.signature, &pubkey)?;
        drop(apk);
        // Returns only if it fails: Android closes the app to replace it.
        run::<R, ()>(app, "install", json!({ "path": path })).await
    }
}

/// The newer version of this app, if one is published.
#[tauri::command]
async fn check<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    platform::check(&app).await
}

/// Downloads the newer version, checks its signature and installs it. On
/// success the app restarts (computers) or closes (Android), so this returns
/// only when there was nothing to install or it failed.
#[tauri::command]
async fn install<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    platform::install(&app).await
}

/// Adds in-app updates to an app: `tauri_plugin_uuob_update::add(tauri::Builder::default())`.
pub fn add<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder.plugin(plugin())
}

fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("uuob-update")
        .invoke_handler(tauri::generate_handler![check, install])
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            platform::setup(_app, _api)?;
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    /// Computers read the feed with Tauri's updater and phones with `newer`:
    /// both have to make sense of what docker/feed.py publishes.
    #[test]
    fn the_published_feed_is_readable() {
        let dist = std::env::temp_dir().join(format!("uuob-feed-{}", std::process::id()));
        std::fs::create_dir_all(dist.join("releases")).unwrap();
        for file in ["arm64.apk.sig", "mac.app.tar.gz.sig", "mac.dmg"] {
            std::fs::write(dist.join(format!("UUOB-0.1.2-{file}")), "s").unwrap();
        }
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docker/feed.py");
        let written = std::process::Command::new("python3").args([script, "someone/releases", "uuob", "UUOB", "0.1.2"]).arg(&dist).status().unwrap();
        assert!(written.success());
        let feed = std::fs::read(dist.join("releases/uuob.json")).unwrap();
        std::fs::remove_dir_all(dist).unwrap();

        let release: tauri_plugin_updater::RemoteRelease = serde_json::from_slice(&feed).unwrap();
        assert_eq!(release.version, semver::Version::new(0, 1, 2));
        let (version, file) = crate::feed::newer(&feed, &semver::Version::new(0, 1, 1), "android-aarch64").unwrap().unwrap();
        assert_eq!(version, "0.1.2");
        assert_eq!(file.url, "https://github.com/someone/releases/releases/download/uuob-v0.1.2/UUOB-0.1.2-arm64.apk");
    }
}
