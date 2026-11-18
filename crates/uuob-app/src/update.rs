//! "Update to …": UUOB installs newer versions of itself. It uses the same
//! feed, and checks the same signatures, as UUOB did when it was a Tauri app
//! and as the apps still do (`crates/uuob-update`), so installed copies of any
//! version find the next one.
//!
//! - Mac: the signed `.app.tar.gz` replaces this `UUOB.app`; the app restarts.
//! - Android: the signed APK is saved for the system installer (`Updater.kt`).

#[path = "../../uuob-update/src/feed.rs"]
mod feed;

use serde_json::Value as Json;

/// Where UUOB's updates are published. A test build may name another feed
/// (`UUOB_UPDATE_FEED` when it is built).
const FEED: &str = match option_env!("UUOB_UPDATE_FEED") {
    Some(feed) => feed,
    None => "https://raw.githubusercontent.com/jachymri/uuob-releases/main/uuob.json",
};
/// The key UUOB's updates are signed with (minisign, as Tauri's signer writes it).
const KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDU5QjY3MTNBQjA5NTI0QzAKUldUQUpKV3dPbkcyV2VzZU9XSWJQT29NWTR3angxNjU1VWJZazJsMi80Zjl6WDJoSWxMK0t3MlUK";

/// This device's name in the feed: `darwin-aarch64`, `android-aarch64`, …
fn target() -> String {
    let os = if cfg!(target_os = "macos") { "darwin" } else { std::env::consts::OS };
    format!("{os}-{}", std::env::consts::ARCH)
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let failed = |e: ureq::Error| format!("The update couldn’t be downloaded ({e}).");
    let response = ureq::get(url).call().map_err(failed)?;
    response.into_body().with_config().limit(1 << 30).read_to_vec().map_err(failed)
}

/// The newer version and its file for this device, if one is published.
fn newer() -> Result<Option<(String, feed::File)>, String> {
    let current = semver::Version::parse(crate::VERSION).map_err(|e| e.to_string())?;
    feed::newer(&get(FEED)?, &current, &target())
}

/// The newer version, if there is one.
pub fn check() -> Result<Option<String>, String> {
    Ok(newer()?.map(|(version, _)| version))
}

/// Downloads the newer version and checks its signature, then installs it on
/// a Mac (`{"restart": "<the app>"}`) or saves it for the system installer on
/// Android (`{"apk": "<file>"}`, into `params.dir`). Null when there is
/// nothing newer.
pub fn install(params: &Json) -> Result<Json, String> {
    let Some((_, file)) = newer()? else { return Ok(Json::Null) };
    let data = get(&file.url)?;
    feed::verify(&data, &file.signature, KEY)?;
    platform::install(data, params)
}

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    use serde_json::json;

    pub fn install(apk: Vec<u8>, params: &Json) -> Result<Json, String> {
        let dir = params["dir"].as_str().ok_or("where to save the update is missing")?;
        let path = std::path::Path::new(dir).join("update.apk");
        std::fs::write(&path, apk).map_err(|e| e.to_string())?;
        Ok(json!({ "apk": path }))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    /// Unpacks the new `UUOB.app` next to this one and swaps the two.
    pub fn install(archive: Vec<u8>, _: &Json) -> Result<Json, String> {
        let text = |e: std::io::Error| format!("The update couldn’t be installed ({e}).");
        let exe = std::env::current_exe().map_err(text)?;
        // …/UUOB.app/Contents/MacOS/UUOB
        let app = exe.ancestors().nth(3).filter(|a| a.extension().is_some_and(|e| e == "app")).ok_or("UUOB isn’t running as an app")?;
        let folder = app.parent().ok_or("UUOB isn’t running as an app")?;
        let staging = folder.join(".uuob-update");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(text)?;
        let swapped = (|| {
            tar::Archive::new(flate2::read::GzDecoder::new(&archive[..])).unpack(&staging)?;
            let new = find_app(&staging)?;
            let old = staging.join("previous.app");
            std::fs::rename(app, &old)?;
            if let Err(e) = std::fs::rename(&new, app) {
                let _ = std::fs::rename(&old, app);
                return Err(e);
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&staging);
        swapped.map_err(text)?;
        Ok(json!({ "restart": app }))
    }

    fn find_app(folder: &Path) -> std::io::Result<std::path::PathBuf> {
        std::fs::read_dir(folder)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or_else(|| std::io::Error::other("the update has no app in it"))
    }
}

#[cfg(not(any(target_os = "android", target_os = "macos")))]
mod platform {
    use super::*;

    pub fn install(_: Vec<u8>, _: &Json) -> Result<Json, String> {
        Err("On Linux, update UUOB with its install line (see docs/setup.md).".into())
    }
}
