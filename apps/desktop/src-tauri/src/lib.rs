#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
  app.package_info().version.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![app_version])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
