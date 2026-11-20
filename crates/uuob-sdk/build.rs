fn main() {
    tauri_plugin::Builder::new(&["port", "link", "removed", "prove"]).android_path("android").build();
}
