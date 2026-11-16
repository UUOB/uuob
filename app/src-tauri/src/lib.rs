//! The UUOB app: the node and the local server (for apps and AI clients),
//! plus a window to manage devices, apps and access. It keeps running in the
//! background (tray / menu bar on computers, a notification on Android) so
//! apps can use it and your devices stay in sync.

use serde_json::Value as Json;
use tauri::{Emitter, Manager, State};
use uuob_core::{Client, Config, Event, NetMode, Node};

/// The UUOB window acting for the user.
#[tauri::command]
async fn call(node: State<'_, Node>, method: String, params: Json) -> Result<Json, String> {
    node.call(&Client::Shell, &method, params).await.map_err(|e| format!("{}: {e}", e.code()))
}

/// The local MCP server's URL, or null on phones (AI clients run on computers).
#[tauri::command]
fn mcp_url() -> Option<String> {
    cfg!(desktop).then(|| format!("http://127.0.0.1:{}/mcp", uuob_api::port()))
}

/// Why the local server couldn't start (e.g. the port is taken), if it didn't.
static SERVER_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[tauri::command]
fn server_error() -> Option<String> {
    SERVER_ERROR.lock().unwrap().clone()
}

/// The window, once there is one, to tell it what changed.
static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// The node and its local server (the API for apps, MCP for AI clients).
/// There is one per process, started by whoever needs it first: the window
/// or, on a phone that was just turned on, the background service.
async fn node(data_dir: std::path::PathBuf) -> uuob_core::Result<Node> {
    static NODE: tokio::sync::OnceCell<Node> = tokio::sync::OnceCell::const_new();
    let open = || async move {
        let node = Node::open(Config {
            data_dir,
            device_name: uuob_core::device_name(),
            platform: std::env::consts::OS.into(),
            net: NetMode::Internet,
            keystore: None,
        })
        .await?;
        let n = node.clone();
        tauri::async_runtime::spawn(async move {
            let addr = ([127, 0, 0, 1], uuob_api::port()).into();
            if let Err(e) = uuob_api::serve(n, addr).await {
                eprintln!("local server not started: {e}");
                *SERVER_ERROR.lock().unwrap() = Some(format!("Apps can’t connect: port {} is in use ({e}).", uuob_api::port()));
                if let Some(app) = APP.get() {
                    let _ = app.emit("uuob", Event::NetworkChanged);
                }
            }
        });
        Ok(node)
    };
    NODE.get_or_try_init(open).await.cloned()
}

/// Android: the background service starts the node as well, so UUOB runs
/// after the phone was turned on, before its screen is ever opened.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_uuob_UuobService_startNode(mut env: jni::JNIEnv, _this: jni::objects::JObject, data_dir: jni::objects::JString) {
    let Ok(data_dir) = env.get_string(&data_dir) else { return };
    let data_dir = std::path::PathBuf::from(String::from(data_dir));
    tauri::async_runtime::spawn(async move {
        if let Err(e) = node(data_dir).await {
            eprintln!("UUOB could not start: {e}");
        }
    });
}

/// Android: UUOB starts when the phone turns on if its `BootReceiver` is
/// switched on (it is off until the user asks). Changes that if asked to,
/// and says how it is now.
#[cfg(target_os = "android")]
fn boot_receiver(set: Option<bool>) -> jni::errors::Result<bool> {
    use jni::objects::{JObject, JValue};
    let android = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(android.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let found = env.with_local_frame(8, |env| {
        let context = unsafe { JObject::from_raw(android.context().cast()) };
        let name = env.new_string("app.uuob.BootReceiver")?;
        let receiver = env.new_object(
            "android/content/ComponentName",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(&context), JValue::Object(&name)],
        )?;
        let packages = env.call_method(&context, "getPackageManager", "()Landroid/content/pm/PackageManager;", &[])?.l()?;
        // PackageManager: 1 is "enabled", 0 "as the app came" (off); the last 1 is "don't restart the app".
        if let Some(on) = set {
            let args = [JValue::Object(&receiver), JValue::Int(on as i32), JValue::Int(1)];
            env.call_method(&packages, "setComponentEnabledSetting", "(Landroid/content/ComponentName;II)V", &args)?;
        }
        let state = env.call_method(&packages, "getComponentEnabledSetting", "(Landroid/content/ComponentName;)I", &[JValue::Object(&receiver)])?;
        Ok(state.i()? == 1)
    });
    if found.is_err() {
        let _ = env.exception_clear();
    }
    found
}

/// Where this computer keeps "start UUOB when I log in": a LaunchAgent on a
/// Mac, an autostart entry on Linux. `None` elsewhere (Android keeps the
/// choice itself, see `boot_receiver`).
fn autostart_file() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    if cfg!(target_os = "macos") {
        Some(home.join("Library/LaunchAgents/app.uuob.plist"))
    } else if cfg!(target_os = "linux") {
        let config = std::env::var_os("XDG_CONFIG_HOME").map(Into::into).unwrap_or_else(|| home.join(".config"));
        Some(config.join("autostart/uuob.desktop"))
    } else {
        None
    }
}

/// Does UUOB start by itself when the user logs in, or when the phone turns
/// on? Null where there is no such choice.
#[tauri::command]
fn autostart() -> Option<bool> {
    #[cfg(target_os = "android")]
    return boot_receiver(None).ok();
    #[allow(unreachable_code)]
    autostart_file().map(|f| f.exists())
}

/// Start UUOB when the user logs in (in the background: no window, just the
/// tray / menu bar icon) or when the phone turns on (just its notification),
/// or stop doing that.
#[tauri::command]
fn set_autostart(on: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return boot_receiver(Some(on)).map(drop).map_err(|e| e.to_string());
    #[allow(unreachable_code)]
    let file = autostart_file().ok_or("this device decides that itself")?;
    if !on {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    // An AppImage runs from a folder that is gone after a restart; start the file itself.
    let program = match std::env::var_os("APPIMAGE") {
        Some(appimage) => std::path::PathBuf::from(appimage),
        None => std::env::current_exe().map_err(|e| e.to_string())?,
    };
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
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, entry).map_err(|e| e.to_string())
}

#[cfg(desktop)]
fn show_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(desktop)]
fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;
    let open = MenuItem::with_id(app, "open", "Open UUOB", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit UUOB", true, None::<&str>)?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("UUOB")
        .menu(&Menu::with_items(app, &[&open, &quit])?)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Android: UUOB's process outlives its screen (see `ExitRequested` below).
/// When the screen is opened again, a new Android activity appears with
/// nothing in it, so `MainActivity` tells us and we give it a window again.
#[cfg(target_os = "android")]
static SCREEN_CREATED: std::sync::OnceLock<tokio::sync::mpsc::UnboundedSender<()>> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_uuob_MainActivity_screenCreated(_env: *mut std::ffi::c_void, _this: *mut std::ffi::c_void) {
    // Not set yet on the very first launch: Tauri creates that window itself.
    if let Some(tx) = SCREEN_CREATED.get() {
        let _ = tx.send(());
    }
}

#[cfg(target_os = "android")]
fn rebuild_window_when_reopened(app: tauri::AppHandle) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _ = SCREEN_CREATED.set(tx);
    tauri::async_runtime::spawn(async move {
        while rx.recv().await.is_some() {
            // The old window may still be closing; a rotated screen keeps its window.
            for _ in 0..15 {
                if app.get_webview_window("main").is_some() {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
                let built = tauri::WebviewWindowBuilder::from_config(&app, &app.config().app.windows[0])
                    .and_then(|b| b.build());
                match built {
                    Ok(_) => break,
                    Err(e) => {
                        eprintln!("could not rebuild the window yet: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,iroh=warn,noq=warn".into()))
        .try_init();

    // "Update to …": the app installs newer versions of itself.
    let builder = tauri_plugin_uuob_update::add(tauri::Builder::default());
    // Opening UUOB again just shows the running one.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| show_window(app)));
    builder
        .setup(|app| {
            let _ = APP.set(app.handle().clone());
            let data_dir = std::env::var_os("UUOB_DATA_DIR")
                .map(Into::into)
                .unwrap_or(app.path().app_data_dir()?);
            let node = tauri::async_runtime::block_on(node(data_dir))?;

            // Forward node events to the window; bring it up when an app asks something.
            let mut events = node.subscribe();
            let handle = app.handle().clone();
            let n = node.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::sync::broadcast::error::RecvError;
                loop {
                    match events.recv().await {
                        Ok(e) => {
                            #[cfg(desktop)]
                            if matches!(e, Event::AsksChanged) && n.asks().iter().any(|a| a.here) {
                                show_window(&handle);
                            }
                            let _ = handle.emit("uuob", e);
                        }
                        // Fell behind: don't stop, tell the window to reload.
                        Err(RecvError::Lagged(_)) => {
                            let _ = handle.emit("uuob", Event::NetworkChanged);
                            let _ = handle.emit("uuob", Event::AsksChanged);
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
                drop(n);
            });

            #[cfg(desktop)]
            if std::env::var_os("UUOB_NO_TRAY").is_none() {
                tray(app)?;
            }
            #[cfg(target_os = "android")]
            rebuild_window_when_reopened(app.handle().clone());
            // Started at login (see `set_autostart`), UUOB stays in the
            // background; started by the user, it shows its window.
            #[cfg(desktop)]
            if !std::env::args().any(|a| a == "--hidden") {
                show_window(app.handle());
            }
            app.manage(node);
            Ok(())
        })
        // Closing the window keeps UUOB running in the background.
        .on_window_event(|w, e| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                if std::env::var_os("UUOB_NO_TRAY").is_none() {
                    api.prevent_close();
                    let _ = w.hide();
                }
            }
            let _ = (w, e);
        })
        .invoke_handler(tauri::generate_handler![call, mcp_url, server_error, autostart, set_autostart])
        .build(tauri::generate_context!())
        .expect("error while starting UUOB")
        .run(|app, e| {
            // macOS: clicking the Dock icon brings the window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = e {
                show_window(app);
            }
            // Android: when the screen goes away (swiped from Recents, or
            // closed to save memory) keep the process, and with it the node
            // that apps and other devices use. The background service keeps
            // Android from killing it.
            #[cfg(mobile)]
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = &e {
                api.prevent_exit();
            }
            let _ = (app, e);
        });
}
