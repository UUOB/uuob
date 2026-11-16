//! Tasks: just a window. It has no backend of its own; its data lives in UUOB,
//! which it reaches over UUOB's local API (see ui/uuob.js).

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // "Update to …": the app installs newer versions of itself.
    tauri_plugin_uuob_update::add(tauri::Builder::default())
        // Phones: bring UUOB to the front when the user needs to tap Allow there.
        .plugin(tauri_plugin_opener::init())
        // Where UUOB is: on this device, or on another one the user linked.
        .plugin(tauri_plugin_uuob_sdk::init())
        .run(tauri::generate_context!())
        .expect("error while running Tasks");
}
