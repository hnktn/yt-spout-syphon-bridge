/// Software Rendering ベースのプレビューモジュール（macOS 専用）
///
/// ## 実装方針
/// mpv の SW レンダラーを使用して CPU メモリに直接 RGBA フレームを描画し、
/// base64 エンコードして Tauri Event で WebView に送信する。
/// OpenGL/Metal を使わないシンプルな実装。

use anyhow::Result;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// レンダリングスレッドへの制御コマンド
pub enum RenderCommand {
    Stop,
}

/// プレビューハンドル
pub struct PreviewHandle {
    pub cmd_tx: mpsc::Sender<RenderCommand>,
}

impl PreviewHandle {
    pub fn stop(&self) {
        let _ = self.cmd_tx.send(RenderCommand::Stop);
    }
}

/// mpv ハンドルポインタのラッパー（スレッド間移動用）
struct SendableMpvHandle(*mut libmpv2_sys::mpv_handle);
unsafe impl Send for SendableMpvHandle {}

/// SW レンダリングベースのプレビューを別スレッドで起動する
///
/// # 引数
/// * `mpv_handle` - mpv 内部ハンドルの生ポインタ
/// * `app_handle` - Tauri AppHandle（Event 送信用）
/// * `width` / `height` - プレビュー解像度
pub fn spawn(
    mpv_handle: *mut libmpv2_sys::mpv_handle,
    app_handle: AppHandle,
    width: u32,
    height: u32,
) -> Result<PreviewHandle> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<RenderCommand>();
    let sendable = SendableMpvHandle(mpv_handle);

    std::thread::spawn(move || {
        if let Err(e) = render_loop_sw(sendable, app_handle, cmd_rx, width, height) {
            log::error!("SW レンダリングループでエラー: {}", e);
        }
    });

    Ok(PreviewHandle { cmd_tx })
}

/// Software Rendering ループ
///
/// mpv → CPU メモリ (RGBA) → base64 → Tauri Event
fn render_loop_sw(
    mpv_handle: SendableMpvHandle,
    app_handle: AppHandle,
    cmd_rx: mpsc::Receiver<RenderCommand>,
    width: u32,
    height: u32,
) -> Result<()> {
    use libmpv2::Mpv;

    // mpv インスタンスを取得
    let mpv = unsafe { Mpv::from_raw(libmpv2_sys::mpv_create_client(mpv_handle.0, std::ptr::null())) };

    // Software Rendering を設定
    mpv.set_property("vo", "null")?; // ビデオ出力を無効化（スクリーンショットで代用）
    mpv.set_property("hwdec", "no")?; // ハードウェアデコードを無効化

    log::info!("SW レンダリング開始: {}x{}", width, height);

    // ピクセルバッファ（RGBA8）
    let pixel_count = (width * height * 4) as usize;
    let mut pixels = vec![0u8; pixel_count];

    // フレーム送信間隔（15fps = 66ms）
    let frame_interval = Duration::from_millis(66);
    let mut last_emit = Instant::now();

    // レンダリングループ
    loop {
        // 停止コマンドが届いたら終了
        if let Ok(RenderCommand::Stop) = cmd_rx.try_recv() {
            break;
        }

        // 一定間隔でスクリーンショットを撮影して WebView に送信
        if last_emit.elapsed() >= frame_interval {
            // mpv のスクリーンショット機能を使ってフレームを取得
            // NOTE: このアプローチは非効率ですが、シンプルで安定しています
            // 実際の製品版では libmpv の render API を使用すべきです

            // TODO: mpv のスクリーンショット API を使ってフレームを取得
            // 現時点では空のフレームを送信（実装の骨組みとして）
            pixels.fill(0);

            // Tauri Event で WebView に送信（base64 エンコード）
            let b64 = base64_encode_pixels(&pixels);
            let _ = app_handle.emit("preview-frame", PreviewFramePayload { data: b64 });

            last_emit = Instant::now();
        }

        // 60fps ターゲットでポーリング
        std::thread::sleep(Duration::from_millis(16));
    }

    log::info!("SW レンダリングを終了しました");
    Ok(())
}

/// ピクセルデータを base64 エンコードする（WebView 転送用）
fn base64_encode_pixels(pixels: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(pixels)
}

/// Tauri Event で送るペイロード
#[derive(Clone, serde::Serialize)]
struct PreviewFramePayload {
    /// base64 エンコードされた RGBA ピクセルデータ
    data: String,
}
