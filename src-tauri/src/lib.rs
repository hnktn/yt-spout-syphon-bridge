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
            commands::set_mute,
            commands::get_mute,
            commands::set_loop,
            commands::get_loop,
            commands::seek,
            commands::get_time_pos,
            commands::get_duration,
            commands::set_speed,
            commands::get_speed,
            commands::get_media_title,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
