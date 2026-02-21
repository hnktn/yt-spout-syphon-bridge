mod mpv_context;
pub mod audio;

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::output::preview::PreviewHandle;
#[cfg(target_os = "macos")]
use crate::output::syphon::{self, SyphonHandle};
pub use mpv_context::MpvContext;

// ─── プレイヤーの状態 ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PlayStatus {
    Idle,
    Loading,
    Playing,
    Paused,
    Error(String),
}

/// Tauri の `manage()` に渡す共有状態
/// Arc<Mutex<>> で複数スレッドから安全にアクセス
pub struct PlayerState {
    inner: Arc<Mutex<PlayerInner>>,
    /// Tauri AppHandle（プレビューイベント送信用）
    app_handle: Option<tauri::AppHandle>,
}

struct PlayerInner {
    mpv: Option<MpvContext>,
    /// プレビューウィンドウのハンドル（停止時に使う）
    preview: Option<PreviewHandle>,
    /// Syphon 出力ハンドル (macOS のみ)
    #[cfg(target_os = "macos")]
    syphon: Option<SyphonHandle>,
    status: PlayStatus,
    current_url: Option<String>,
    output_active: bool,
}

/// プレビューウィンドウの解像度
const PREVIEW_WIDTH: u32 = 1280;
const PREVIEW_HEIGHT: u32 = 720;

impl PlayerState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PlayerInner {
                mpv: None,
                preview: None,
                #[cfg(target_os = "macos")]
                syphon: None,
                status: PlayStatus::Idle,
                current_url: None,
                output_active: false,
            })),
            app_handle: None,
        }
    }

    /// Tauri AppHandle を設定する（setup 時に呼ぶ）
    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        self.app_handle = Some(handle);
    }

    // ─── 再生制御 ─────────────────────────────────────────────────────────────

    pub async fn play(&self, url: &str, quality: Option<&str>) -> Result<()> {
        println!("=== play() called with URL: {} ===", url);
        let mut inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;

        // 既存のセッションをクリア（プレビューウィンドウと Syphon を停止）
        if let Some(prev) = inner.preview.take() {
            prev.stop();
        }
        #[cfg(target_os = "macos")]
        if let Some(syphon) = inner.syphon.take() {
            syphon.stop();
        }
        inner.mpv = None;
        inner.output_active = false;

        println!("mpv を初期化: URL={}", url);
        log::info!("mpv を初期化: URL={}", url);

        // mpv を初期化して再生開始
        let ctx = MpvContext::new(url, quality)?;

        // Syphon 出力を別スレッドで起動する (macOS のみ)
        // Syphon スレッド内で RenderContext を作成してから loadfile を実行する
        // app_handle を渡すことで、プレビューも Syphon から直接送信される
        #[cfg(target_os = "macos")]
        {
            let handle_ptr = ctx.mpv_handle_ptr();
            let app_clone = self.app_handle.clone();
            let server_name = "yt-spout-syphon-bridge";

            match syphon::spawn(handle_ptr, server_name, url, PREVIEW_WIDTH, PREVIEW_HEIGHT, app_clone) {
                Ok(handle) => {
                    inner.syphon = Some(handle);
                    log::info!("Syphon 出力を起動しました (サーバー名: {})", server_name);
                }
                Err(e) => {
                    log::warn!("Syphon 出力の起動に失敗（再生は続行）: {}", e);
                }
            }
        }

        log::info!("プレビューは Syphon 出力から直接送信されます");

        inner.mpv = Some(ctx);
        inner.status = PlayStatus::Loading;
        inner.current_url = Some(url.to_string());
        inner.output_active = true;

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        // プレビューウィンドウを停止
        if let Some(prev) = inner.preview.take() {
            prev.stop();
        }
        // Syphon 出力を停止 (macOS のみ)
        #[cfg(target_os = "macos")]
        if let Some(syphon) = inner.syphon.take() {
            syphon.stop();
        }
        inner.mpv = None;
        inner.status = PlayStatus::Idle;
        inner.current_url = None;
        inner.output_active = false;
        Ok(())
    }

    pub async fn toggle_pause(&self) -> Result<bool> {
        let mut inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            let paused: bool = mpv.toggle_pause()?;
            inner.status = if paused {
                PlayStatus::Paused
            } else {
                PlayStatus::Playing
            };
            return Ok(paused);
        }
        Ok(false)
    }

    // ─── 状態の読み取り ───────────────────────────────────────────────────────

    pub fn status(&self) -> PlayStatus {
        self.inner.lock()
            .map(|inner| inner.status.clone())
            .unwrap_or(PlayStatus::Error("Mutex ロック失敗".to_string()))
    }

    pub fn current_url(&self) -> Option<String> {
        self.inner.lock()
            .ok()
            .and_then(|inner| inner.current_url.clone())
    }

    pub fn is_output_active(&self) -> bool {
        self.inner.lock()
            .map(|inner| inner.output_active)
            .unwrap_or(false)
    }

    // ─── オーディオ制御 ───────────────────────────────────────────────────────

    pub fn list_audio_devices(&self) -> Vec<(String, String)> {
        let inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(e) => {
                log::error!("Mutex ロック失敗: {}", e);
                return audio::enumerate_devices();
            }
        };
        if let Some(mpv) = &inner.mpv {
            log::info!("mpv からデバイス一覧を取得します");
            match mpv.list_audio_devices() {
                Ok(devices) => {
                    log::info!("mpv から {} 個のデバイスを取得しました", devices.len());
                    devices
                }
                Err(e) => {
                    log::error!("mpv からのデバイス取得に失敗: {}", e);
                    audio::enumerate_devices()
                }
            }
        } else {
            log::info!("mpv が起動していないため、フォールバック関数を使用します");
            // mpv が起動していない場合でもリストを返す
            audio::enumerate_devices()
        }
    }

    pub async fn set_audio_device(&self, device_id: &str) -> Result<()> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            mpv.set_audio_device(device_id).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(())
    }

    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            mpv.set_volume(volume).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(())
    }

    // ─── プレイヤー制御の拡張機能 ─────────────────────────────────────────────

    pub async fn set_loop(&self, enabled: bool) -> Result<()> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            mpv.set_loop(enabled).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(())
    }

    pub fn get_loop(&self) -> Result<bool> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            return mpv.get_loop().map_err(|e| anyhow::anyhow!("{}", e));
        }
        Ok(false)
    }

    pub async fn seek(&self, seconds: f64) -> Result<()> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            mpv.seek(seconds).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(())
    }

    pub fn get_time_pos(&self) -> Result<f64> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            return mpv.get_time_pos().map_err(|e| anyhow::anyhow!("{}", e));
        }
        Ok(0.0)
    }

    pub fn get_duration(&self) -> Result<f64> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            return mpv.get_duration().map_err(|e| anyhow::anyhow!("{}", e));
        }
        Ok(0.0)
    }

    pub async fn set_speed(&self, speed: f64) -> Result<()> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            mpv.set_speed(speed).map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Ok(())
    }

    pub fn get_speed(&self) -> Result<f64> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            return mpv.get_speed().map_err(|e| anyhow::anyhow!("{}", e));
        }
        Ok(1.0)
    }

    pub fn get_media_title(&self) -> Result<String> {
        let inner = self.inner.lock()
            .map_err(|e| anyhow::anyhow!("Mutex ロック失敗: {}", e))?;
        if let Some(mpv) = &inner.mpv {
            return mpv.get_media_title().map_err(|e| anyhow::anyhow!("{}", e));
        }
        Ok(String::new())
    }
}
