mod commands;
mod player;
pub mod output;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // アプリ状態の初期化
            let mut player_state = player::PlayerState::new();
            player_state.set_app_handle(app.handle().clone());
            app.manage(player_state);

            log::info!("yt-spout-syphon-bridge started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::play,
            commands::stop,
            commands::pause,
            commands::get_status,
            commands::get_audio_devices,
            commands::set_audio_device,
            commands::set_volume,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
