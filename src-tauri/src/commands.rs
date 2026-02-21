use crate::player::{PlayerState, PlayStatus};
use serde::{Deserialize, Serialize};
use tauri::State;

/// フロントエンドから受け取るURL
#[derive(Debug, Deserialize)]
pub struct PlayRequest {
    pub url: String,
    /// 任意: 最大解像度 (例: "1080p", "720p", "best")
    pub quality: Option<String>,
}

/// フロントエンドに返すステータス
#[derive(Debug, Serialize, Clone)]
pub struct StatusResponse {
    pub status: String,       // "idle" | "loading" | "playing" | "paused" | "error"
    pub url: Option<String>,
    pub error: Option<String>,
    pub spout_active: bool,
    pub syphon_active: bool,
}

/// オーディオデバイス情報
#[derive(Debug, Serialize, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

// ─── Tauri IPC コマンド ───────────────────────────────────────────────────────

/// YouTube URL を受け取り、ストリーミング再生 + Spout/Syphon 出力を開始する
#[tauri::command]
pub async fn play(
    request: PlayRequest,
    state: State<'_, PlayerState>,
) -> Result<StatusResponse, String> {
    log::info!("play command: url={}", request.url);

    state
        .play(&request.url, request.quality.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    Ok(StatusResponse {
        status: "loading".to_string(),
        url: Some(request.url),
        error: None,
        spout_active: state.is_output_active(),
        syphon_active: state.is_output_active(),
    })
}

/// 再生を停止し、Spout/Syphon 出力をクリアする
#[tauri::command]
pub async fn stop(state: State<'_, PlayerState>) -> Result<StatusResponse, String> {
    log::info!("stop command");

    state.stop().await.map_err(|e| e.to_string())?;

    Ok(StatusResponse {
        status: "idle".to_string(),
        url: None,
        error: None,
        spout_active: false,
        syphon_active: false,
    })
}

/// 一時停止 / 再開トグル
#[tauri::command]
pub async fn pause(state: State<'_, PlayerState>) -> Result<StatusResponse, String> {
    let paused = state.toggle_pause().await.map_err(|e| e.to_string())?;

    let status_str = if paused { "paused" } else { "playing" };
    Ok(StatusResponse {
        status: status_str.to_string(),
        url: state.current_url(),
        error: None,
        spout_active: state.is_output_active(),
        syphon_active: state.is_output_active(),
    })
}

/// 現在のプレイヤーステータスを取得する
#[tauri::command]
pub fn get_status(state: State<'_, PlayerState>) -> StatusResponse {
    let play_status = state.status();
    StatusResponse {
        status: match play_status {
            PlayStatus::Idle => "idle",
            PlayStatus::Loading => "loading",
            PlayStatus::Playing => "playing",
            PlayStatus::Paused => "paused",
            PlayStatus::Error(_) => "error",
        }
        .to_string(),
        url: state.current_url(),
        error: match play_status {
            PlayStatus::Error(e) => Some(e),
            _ => None,
        },
        spout_active: state.is_output_active(),
        syphon_active: state.is_output_active(),
    }
}

/// システムのオーディオデバイス一覧を取得する
#[tauri::command]
pub fn get_audio_devices(state: State<'_, PlayerState>) -> Vec<AudioDevice> {
    state
        .list_audio_devices()
        .into_iter()
        .map(|(id, name)| AudioDevice { id, name })
        .collect()
}

/// 出力オーディオデバイスを切り替える
/// device_id: "" を渡すとデフォルトデバイスにリセット
#[tauri::command]
pub async fn set_audio_device(
    device_id: String,
    state: State<'_, PlayerState>,
) -> Result<(), String> {
    state
        .set_audio_device(&device_id)
        .await
        .map_err(|e| e.to_string())
}

/// ボリューム設定 (0–100)
#[tauri::command]
pub async fn set_volume(volume: u8, state: State<'_, PlayerState>) -> Result<(), String> {
    state
        .set_volume(volume)
        .await
        .map_err(|e| e.to_string())
}

// ─── プレイヤー制御の拡張機能 ─────────────────────────────────────────────

/// ループ再生を設定
#[tauri::command]
pub async fn set_loop(enabled: bool, state: State<'_, PlayerState>) -> Result<(), String> {
    state.set_loop(enabled).await.map_err(|e| e.to_string())
}

/// ループ再生の状態を取得
#[tauri::command]
pub fn get_loop(state: State<'_, PlayerState>) -> Result<bool, String> {
    state.get_loop().map_err(|e| e.to_string())
}

/// シーク（秒単位）
#[tauri::command]
pub async fn seek(seconds: f64, state: State<'_, PlayerState>) -> Result<(), String> {
    state.seek(seconds).await.map_err(|e| e.to_string())
}

/// 再生位置を取得（秒）
#[tauri::command]
pub fn get_time_pos(state: State<'_, PlayerState>) -> Result<f64, String> {
    state.get_time_pos().map_err(|e| e.to_string())
}

/// 総再生時間を取得（秒）
#[tauri::command]
pub fn get_duration(state: State<'_, PlayerState>) -> Result<f64, String> {
    state.get_duration().map_err(|e| e.to_string())
}

/// 再生速度を設定（0.25 〜 4.0）
#[tauri::command]
pub async fn set_speed(speed: f64, state: State<'_, PlayerState>) -> Result<(), String> {
    state.set_speed(speed).await.map_err(|e| e.to_string())
}

/// 再生速度を取得
#[tauri::command]
pub fn get_speed(state: State<'_, PlayerState>) -> Result<f64, String> {
    state.get_speed().map_err(|e| e.to_string())
}

/// 動画タイトルを取得
#[tauri::command]
pub fn get_media_title(state: State<'_, PlayerState>) -> Result<String, String> {
    state.get_media_title().map_err(|e| e.to_string())
}
