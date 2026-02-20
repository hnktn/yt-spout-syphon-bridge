/// プレビューモジュール（WebView Canvas 転送版）
///
/// ## 実装方針
/// macOS では winit の EventLoop がメインスレッド制約のため、
/// オフスクリーン OpenGL コンテキストで FBO に描画し、
/// ピクセルデータを読み取って Tauri Event で WebView に送信する。
///
/// フレーム転送は重いため、間引き（例: 15fps）で送信する。
use anyhow::Result;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use raw_window_handle::RawDisplayHandle;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// レンダリングスレッドへの制御コマンド
pub enum RenderCommand {
    /// 停止してスレッドを終了する
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

/// プレビューレンダリングを別スレッドで起動する
///
/// オフスクリーン GL コンテキストで FBO に描画し、
/// ピクセルデータを読み取って Tauri Event で WebView に送る。
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
        if let Err(e) = render_loop_offscreen(sendable, app_handle, cmd_rx, width, height) {
            log::error!("オフスクリーンレンダリングループでエラー: {}", e);
        }
    });

    Ok(PreviewHandle { cmd_tx })
}

/// オフスクリーンレンダリングループ
///
/// glutin でオフスクリーン GL コンテキストを作成し、
/// mpv → FBO → glReadPixels → Tauri Event の流れでフレームを転送する。
fn render_loop_offscreen(
    mpv_handle: SendableMpvHandle,
    app_handle: AppHandle,
    cmd_rx: mpsc::Receiver<RenderCommand>,
    width: u32,
    height: u32,
) -> Result<()> {
    use glutin::config::ConfigTemplateBuilder;
    use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
    use glutin::display::Display;
    use glutin::prelude::*;

    // オフスクリーン用の Display を作成（macOS では CGL）
    let display = unsafe {
        Display::new(
            RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new()),
            glutin::display::DisplayApiPreference::Cgl,
        )?
    };

    // GL コンフィグを選択
    let template = ConfigTemplateBuilder::new().build();
    let config = unsafe {
        display
            .find_configs(template)?
            .reduce(|a, b| if b.num_samples() > a.num_samples() { b } else { a })
            .ok_or_else(|| anyhow::anyhow!("GL config が見つかりません"))?
    };

    // サーフェスレス GL コンテキストを作成（OpenGL 3.3 Core）
    let ctx_attrs = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(None);

    let not_current = unsafe { display.create_context(&config, &ctx_attrs)? };

    // サーフェスレスコンテキストを current にする（macOS では treat_as_possibly_current を使用）
    let _gl_ctx: PossiblyCurrentContext = unsafe { not_current.treat_as_possibly_current() };

    // GL 関数ポインタをロード
    gl::load_with(|name| {
        display
            .get_proc_address(&std::ffi::CString::new(name).unwrap())
            .cast()
    });

    // FBO とテクスチャを作成
    let (fbo, texture) = create_fbo(width, height);

    // mpv の RenderContext を作成
    let gl_display_ptr = &display as *const _ as *const std::ffi::c_void;

    fn get_proc_addr_via_ptr(
        ctx: &*const std::ffi::c_void,
        name: &str,
    ) -> *mut std::ffi::c_void {
        unsafe {
            let display = &*(*ctx as *const Display);
            let name_cstr = std::ffi::CString::new(name).unwrap();
            display.get_proc_address(&name_cstr).cast_mut()
        }
    }

    let render_ctx = unsafe {
        RenderContext::new(
            &mut *mpv_handle.0,
            [
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address: get_proc_addr_via_ptr,
                    ctx: gl_display_ptr,
                }),
            ],
        )
        .map_err(|e| anyhow::anyhow!("RenderContext の作成に失敗: {:?}", e))?
    };

    log::info!("オフスクリーンレンダリング開始: {}x{}", width, height);

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

        // mpv に FBO へ描画させる
        if let Err(e) = render_ctx.render::<()>(fbo as i32, width as i32, height as i32, true) {
            log::warn!("mpv render エラー: {:?}", e);
            std::thread::sleep(Duration::from_millis(16));
            continue;
        }

        // 一定間隔でピクセルデータを読み取って WebView に送信
        if last_emit.elapsed() >= frame_interval {
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
                gl::ReadPixels(
                    0,
                    0,
                    width as _,
                    height as _,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    pixels.as_mut_ptr() as *mut _,
                );
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            }

            // Tauri Event で WebView に送信（base64 エンコード）
            let b64 = base64_encode_pixels(&pixels, width, height);
            let _ = app_handle.emit("preview-frame", PreviewFramePayload { data: b64 });

            last_emit = Instant::now();
        }

        // 60fps ターゲットでポーリング
        std::thread::sleep(Duration::from_millis(16));
    }

    // クリーンアップ
    unsafe {
        gl::DeleteFramebuffers(1, &fbo);
        gl::DeleteTextures(1, &texture);
    }

    log::info!("オフスクリーンレンダリングを終了しました");
    Ok(())
}

/// FBO とテクスチャを作成する
fn create_fbo(width: u32, height: u32) -> (gl::types::GLuint, gl::types::GLuint) {
    let mut fbo: gl::types::GLuint = 0;
    let mut texture: gl::types::GLuint = 0;

    unsafe {
        gl::GenTextures(1, &mut texture);
        gl::BindTexture(gl::TEXTURE_2D, texture);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as _,
            width as _,
            height as _,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

        gl::GenFramebuffers(1, &mut fbo);
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
        gl::FramebufferTexture2D(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            texture,
            0,
        );

        let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
        if status != gl::FRAMEBUFFER_COMPLETE {
            log::error!("FBO が不完全: 0x{:X}", status);
        }

        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }

    (fbo, texture)
}

/// ピクセルデータを base64 エンコードする（WebView 転送用）
///
/// データ URL スキーム形式: `data:image/png;base64,...`
/// （実際は PNG エンコードせず RGBA 生データを送り、Canvas で ImageData として復元する）
fn base64_encode_pixels(pixels: &[u8], _width: u32, _height: u32) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(pixels)
}

/// Tauri Event で送るペイロード
#[derive(Clone, serde::Serialize)]
struct PreviewFramePayload {
    /// base64 エンコードされた RGBA ピクセルデータ
    data: String,
}
