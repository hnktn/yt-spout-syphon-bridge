/// Syphon 出力モジュール（macOS 専用）
///
/// ## 実装方針
/// 1. mpv を OpenGL でレンダリング（FBO にテクスチャを描画）
/// 2. Syphon Server を作成してテクスチャ ID を共有
/// 3. TouchDesigner / VDMX などの Syphon Client で受信

use anyhow::Result;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, Encode, Encoding};
use objc2_foundation::NSString;
use std::sync::mpsc;
use std::time::Duration;
use tauri::Emitter;

// ─── macOS ネイティブ API の FFI 宣言 ──────────────────────────────────────

#[allow(non_camel_case_types)]
type CGLContextObj = *mut std::ffi::c_void;

#[allow(non_camel_case_types)]
type CGLPixelFormatObj = *mut std::ffi::c_void;

#[allow(non_camel_case_types)]
type CGLError = i32;

#[allow(dead_code)]
const CGL_NO_ERROR: CGLError = 0;

#[allow(dead_code)]
const CGL_PFA_ACCELERATED: u32 = 73;
#[allow(dead_code)]
const CGL_PFA_OPENGL_PROFILE: u32 = 99;
#[allow(dead_code)]
const CGL_OGL_VERSION_3_2_CORE: u32 = 0x3200;

extern "C" {
    fn CGLChoosePixelFormat(
        attribs: *const u32,
        pix: *mut CGLPixelFormatObj,
        npix: *mut i32,
    ) -> CGLError;

    fn CGLCreateContext(
        pix: CGLPixelFormatObj,
        share: CGLContextObj,
        ctx: *mut CGLContextObj,
    ) -> CGLError;

    fn CGLDestroyPixelFormat(pix: CGLPixelFormatObj);
    fn CGLDestroyContext(ctx: CGLContextObj);
    fn CGLSetCurrentContext(ctx: CGLContextObj) -> CGLError;
}

// macOS 10.14+ では dlsym を使用する
const RTLD_DEFAULT: *mut std::ffi::c_void = -2isize as *mut std::ffi::c_void;

extern "C" {
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const std::ffi::c_char) -> *mut std::ffi::c_void;
    fn dlopen(filename: *const std::ffi::c_char, flag: i32) -> *mut std::ffi::c_void;
    fn dlerror() -> *const std::ffi::c_char;
}

const RTLD_NOW: i32 = 0x2;
const RTLD_LOCAL: i32 = 0x4;

/// Syphon.framework を明示的にロードする
///
/// objc2::class!(SyphonServer) を使用する前に、framework を dlopen でロードする必要がある。
/// DYLD_FRAMEWORK_PATH だけでは不十分。
fn load_syphon_framework() -> Result<()> {
    unsafe {
        // framework パスを環境変数またはハードコードから取得
        let framework_paths = [
            "/Users/haruhisa/Library/CloudStorage/Dropbox/Repos/yt-spout-syphon-bridge/src-tauri/bindings/syphon/Syphon.framework/Syphon\0",
            "./bindings/syphon/Syphon.framework/Syphon\0",
            "bindings/syphon/Syphon.framework/Syphon\0",
        ];

        for path in &framework_paths {
            log::info!("Syphon.framework をロード中: {}", path.trim_end_matches('\0'));
            let path_cstr = path.as_ptr() as *const std::ffi::c_char;
            let handle = dlopen(path_cstr, RTLD_NOW | RTLD_LOCAL);

            if !handle.is_null() {
                log::info!("Syphon.framework を正常にロードしました: {}", path.trim_end_matches('\0'));
                return Ok(());
            } else {
                let error_ptr = dlerror();
                if !error_ptr.is_null() {
                    let error_cstr = std::ffi::CStr::from_ptr(error_ptr);
                    log::warn!("dlopen エラー ({}): {}", path.trim_end_matches('\0'), error_cstr.to_string_lossy());
                }
            }
        }

        Err(anyhow::anyhow!("Syphon.framework のロードに失敗しました（すべてのパスを試行）"))
    }
}

// ─── Syphon Framework の明示的なリンク ────────────────────────────────────
// objc2::class!(SyphonServer) は実行時の動的呼び出しのため、
// リンカーは Syphon.framework への依存を検出できない。
// build.rs で -Wl,-needed_framework,Syphon を使用して強制的にリンクする。
#[link(name = "Syphon", kind = "framework")]
extern "C" {}

/// レンダリングスレッドへの制御コマンド
pub enum SyphonCommand {
    Stop,
}

/// Syphon 出力ハンドル
pub struct SyphonHandle {
    pub cmd_tx: mpsc::Sender<SyphonCommand>,
    pub thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl SyphonHandle {
    pub fn stop(mut self) {
        // 停止コマンドを送信
        let _ = self.cmd_tx.send(SyphonCommand::Stop);

        // スレッドの終了を待つ（最大3秒）
        if let Some(handle) = self.thread_handle.take() {
            log::info!("Syphon スレッドの終了を待機中...");
            let _ = handle.join();
            log::info!("Syphon スレッドが終了しました");
        }
    }
}

/// mpv ハンドルポインタのラッパー（スレッド間移動用）
struct SendableMpvHandle(*mut libmpv2_sys::mpv_handle);
unsafe impl Send for SendableMpvHandle {}

/// Syphon 出力を別スレッドで起動する
///
/// # 引数
/// * `mpv_handle` - mpv 内部ハンドルの生ポインタ（loadfile 未実行）
/// * `server_name` - Syphon サーバー名（TouchDesigner で識別用）
/// * `url` - 再生する URL（RenderContext 作成後に loadfile を実行）
/// * `width` / `height` - 初期出力解像度（動画ロード後に実際の解像度に調整される）
/// * `app_handle` - Tauri AppHandle（プレビュー用、None の場合はプレビュー無効）
pub fn spawn(
    mpv_handle: *mut libmpv2_sys::mpv_handle,
    server_name: &str,
    url: &str,
    width: u32,
    height: u32,
    app_handle: Option<tauri::AppHandle>,
) -> Result<SyphonHandle> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<SyphonCommand>();
    let sendable = SendableMpvHandle(mpv_handle);
    let server_name = server_name.to_string();
    let url = url.to_string();

    let thread_handle = std::thread::spawn(move || {
        println!("=== Syphon thread started ===");
        if let Err(e) = syphon_loop(sendable, &server_name, &url, cmd_rx, width, height, app_handle) {
            println!("!!! Syphon レンダリングループでエラー: {}", e);
            log::error!("Syphon レンダリングループでエラー: {}", e);
        }
        println!("=== Syphon thread finished ===");
    });

    Ok(SyphonHandle {
        cmd_tx,
        thread_handle: Some(thread_handle),
    })
}

/// Syphon レンダリングループ
///
/// CGL コンテキストで mpv → FBO → Syphon Server → プレビュー送信
fn syphon_loop(
    sendable_handle: SendableMpvHandle,
    server_name: &str,
    url: &str,
    cmd_rx: mpsc::Receiver<SyphonCommand>,
    initial_width: u32,
    initial_height: u32,
    app_handle: Option<tauri::AppHandle>,  // プレビュー機能用
) -> Result<()> {
    println!("=== syphon_loop started: {} ===", url);

    // 既存の mpv ハンドルを取得
    let mpv_handle = sendable_handle.0;
    println!("Using existing mpv handle: {:?}", mpv_handle);

    // CGL コンテキストを作成
    println!("Creating CGL context...");
    let gl_ctx = create_cgl_context()?;
    println!("CGL context created: {:?}", gl_ctx);

    // RenderContext を作成
    let render_ctx = unsafe {
        println!("Setting CGL context...");
        CGLSetCurrentContext(gl_ctx);

        println!("Creating RenderContext...");
        log::info!("RenderContext を作成します...");
        log::info!("GL コンテキスト: {:?}", gl_ctx);

        fn get_proc_addr(_ctx: &*const std::ffi::c_void, name: &str) -> *mut std::ffi::c_void {
            unsafe {
                let name_cstr = std::ffi::CString::new(name).unwrap();
                dlsym(RTLD_DEFAULT, name_cstr.as_ptr())
            }
        }

        let ctx_ptr = &gl_ctx as *const _ as *const std::ffi::c_void;
        println!("Calling RenderContext::new...");
        log::info!("RenderContext::new を呼び出します (mpv_handle: {:?}, ctx_ptr: {:?})", mpv_handle, ctx_ptr);

        let render_ctx = RenderContext::new(
            &mut *mpv_handle,
            [
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address: get_proc_addr,
                    ctx: ctx_ptr,
                }),
            ],
        )
        .map_err(|e| anyhow::anyhow!("RenderContext の作成に失敗: {:?}", e))?;

        println!("RenderContext created successfully");
        log::info!("RenderContext を作成しました");

        render_ctx
    };

    // RenderContext 作成後に loadfile を実行
    println!("Executing loadfile command...");
    unsafe {
        let loadfile_cstr = std::ffi::CString::new("loadfile").unwrap();
        let url_cstr = std::ffi::CString::new(url).unwrap();
        let replace_cstr = std::ffi::CString::new("replace").unwrap();
        let mut args: Vec<*const std::ffi::c_char> = vec![
            loadfile_cstr.as_ptr(),
            url_cstr.as_ptr(),
            replace_cstr.as_ptr(),
            std::ptr::null(),
        ];
        let ret = libmpv2_sys::mpv_command(mpv_handle, args.as_mut_ptr());
        if ret < 0 {
            println!("loadfile command failed: {}", ret);
            return Err(anyhow::anyhow!("loadfile コマンドに失敗: {} (エラーコード: {})", url, ret));
        } else {
            println!("loadfile command executed successfully");
            log::info!("loadfile コマンドを実行: {}", url);
        }
    }

    // 動画の実際の解像度を取得するまで待機
    // VIDEO_RECONFIG イベント (id=11) を待ってから解像度を取得する
    println!("Waiting for video resolution...");
    log::info!("動画の解像度情報を取得中...");
    let (actual_width, actual_height) = unsafe {
        let mut width = 0i64;
        let mut height = 0i64;
        let mut attempts = 0;
        let max_attempts = 300; // 最大30秒待つ（100ms x 300）
        let mut video_reconfig_received = false;

        // MPV_EVENT_VIDEO_RECONFIG = 11
        const MPV_EVENT_VIDEO_RECONFIG: u32 = 11;

        loop {
            // mpv イベントをチェック（ブロッキングなし）
            let event = libmpv2_sys::mpv_wait_event(mpv_handle, 0.0);
            if !event.is_null() {
                let event_id = (*event).event_id;
                if event_id != 0 { // MPV_EVENT_NONE 以外
                    if attempts % 10 == 0 || event_id == MPV_EVENT_VIDEO_RECONFIG {
                        println!("mpv event: id={}", event_id);
                        log::debug!("mpv event: id={}", event_id);
                    }

                    if event_id == MPV_EVENT_VIDEO_RECONFIG {
                        println!("VIDEO_RECONFIG event received");
                        video_reconfig_received = true;
                    }
                }
            }

            // VIDEO_RECONFIG イベントを受信した後に解像度を取得
            if video_reconfig_received {
                let dwidth_cstr = std::ffi::CString::new("width").unwrap();
                let dheight_cstr = std::ffi::CString::new("height").unwrap();

                // MPV_FORMAT_INT64 = 4
                const MPV_FORMAT_INT64: u32 = 4;

                let ret_w = libmpv2_sys::mpv_get_property(
                    mpv_handle,
                    dwidth_cstr.as_ptr(),
                    MPV_FORMAT_INT64,
                    &mut width as *mut i64 as *mut _,
                );
                let ret_h = libmpv2_sys::mpv_get_property(
                    mpv_handle,
                    dheight_cstr.as_ptr(),
                    MPV_FORMAT_INT64,
                    &mut height as *mut i64 as *mut _,
                );

                println!("After VIDEO_RECONFIG: ret_w={}, ret_h={}, width={}, height={}",
                         ret_w, ret_h, width, height);

                if ret_w >= 0 && ret_h >= 0 && width > 0 && height > 0 {
                    println!("Got video resolution: {}x{}", width, height);
                    log::info!("動画の実際の解像度: {}x{}", width, height);

                    // 再生開始イベントを送信（フロントエンドのステータスを "loading" → "playing" に更新）
                    if let Some(app) = &app_handle {
                        #[derive(Clone, serde::Serialize)]
                        struct PlayingEvent {
                            status: String,
                        }
                        let _ = app.emit("player-status", PlayingEvent { status: "playing".to_string() });
                        log::info!("player-status イベントを送信しました (playing)");
                    }

                    break;
                }
            }

            attempts += 1;
            if attempts >= max_attempts {
                println!("Resolution timeout, using initial size: {}x{}", initial_width, initial_height);
                log::warn!(
                    "動画の解像度取得がタイムアウト、初期値を使用: {}x{}",
                    initial_width, initial_height
                );
                width = initial_width as i64;
                height = initial_height as i64;
                break;
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        (width as u32, height as u32)
    };

    // FBO とテクスチャを実際の解像度で作成
    println!("Creating FBO with resolution: {}x{}", actual_width, actual_height);
    let (fbo, texture) = create_fbo(actual_width, actual_height);
    println!("FBO created: fbo={}, texture={}", fbo, texture);

    // Syphon Server を実解像度で作成
    println!("Creating Syphon server with resolution: {}x{}", actual_width, actual_height);
    let syphon_server = create_syphon_server(server_name, gl_ctx)?;
    println!("Syphon server created");

    println!("Starting Syphon rendering loop...");
    log::info!("Syphon レンダリング開始: {} ({}x{})", server_name, actual_width, actual_height);

    // プレビュー用 FBO・テクスチャをループ外で1回だけ作成して再利用する
    let preview_width = 320u32;
    let (mut preview_fbo, mut preview_texture) = (0u32, 0u32);
    unsafe {
        CGLSetCurrentContext(gl_ctx);
        gl::GenFramebuffers(1, &mut preview_fbo);
        gl::GenTextures(1, &mut preview_texture);
        gl::BindTexture(gl::TEXTURE_2D, preview_texture);
        // アスペクト比は動画解像度確定後に合わせるため、ひとまず 320x180 で初期化
        let preview_height = ((actual_height as f32 / actual_width as f32) * preview_width as f32).max(1.0) as u32;
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGB as _, preview_width as _, preview_height as _,
            0, gl::RGB, gl::UNSIGNED_BYTE, std::ptr::null(),
        );
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
        gl::BindFramebuffer(gl::FRAMEBUFFER, preview_fbo);
        gl::FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, preview_texture, 0);
        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
    }

    // レンダリングループ
    let mut consecutive_errors = 0;
    let max_consecutive_errors = 30; // 約0.5秒分のエラーで停止
    let mut frame_count = 0u64;

    loop {
        // 停止コマンドが届いたら終了
        if let Ok(SyphonCommand::Stop) = cmd_rx.try_recv() {
            log::info!("停止コマンドを受信、レンダリングを終了します");
            break;
        }

        unsafe {
            CGLSetCurrentContext(gl_ctx);

            // mpv に FBO へ描画させる
            match render_ctx.render::<()>(fbo as i32, actual_width as i32, actual_height as i32, true) {
                Ok(_) => {
                    consecutive_errors = 0;

                    // 最初のフレームをログ出力
                    if frame_count == 0 {
                        println!("First frame rendered successfully!");
                        log::info!("最初のフレームを描画しました");
                    }

                    // Syphon にテクスチャを公開
                    publish_syphon_frame(&syphon_server, texture, actual_width, actual_height);

                    frame_count += 1;

                    // プレビューを送信（毎フレーム、再利用 FBO を使う）
                    if let Some(ref app) = app_handle {
                        send_preview_frame_blit(app, fbo, actual_width, actual_height, preview_fbo, preview_texture);
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    log::warn!("mpv render エラー ({}/{}): {:?}", consecutive_errors, max_consecutive_errors, e);

                    if consecutive_errors >= max_consecutive_errors {
                        log::error!("連続エラーが上限に達したため、レンダリングを停止します");
                        break;
                    }

                    std::thread::sleep(Duration::from_millis(16));
                    continue;
                }
            }
        }

        // 60fps ターゲット
        std::thread::sleep(Duration::from_millis(16));
    }

    // クリーンアップ（重要: 順序を守る）
    log::info!("クリーンアップを開始します");

    unsafe {
        // 1. GL コンテキストをアクティブにする
        CGLSetCurrentContext(gl_ctx);

        // 2. バッファをクリアするために黒いフレームを複数回送信
        log::info!("バッファクリア用の黒いフレームを送信します");

        // テクスチャを黒でクリア
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
        gl::Viewport(0, 0, actual_width as i32, actual_height as i32);
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::Flush();

        // 黒いフレームを複数回送信（TouchDesigner が確実に受信できるように）
        for i in 0..10 {
            // 毎回クリアして確実に黒にする
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            publish_syphon_frame(&syphon_server, texture, actual_width, actual_height);
            gl::Flush();
            std::thread::sleep(Duration::from_millis(50)); // 少し長めに待つ
            log::debug!("黒フレーム送信 {}/10", i + 1);
        }

        // GL 操作が完了するまで待機
        gl::Finish();

        // クライアント側が黒フレームを受信・処理する時間を確保
        log::info!("黒いフレームの送信が完了しました (クライアント受信待機中...)");
        std::thread::sleep(Duration::from_millis(300));

        // 3. Syphon Server を停止して解放
        log::info!("Syphon Server を停止します");
        // [server stop] を呼び出して内部の GCD キューを適切にクリーンアップ
        let _: () = msg_send![&*syphon_server, stop];
        log::info!("Syphon Server の stop メソッドを呼び出しました");

        // GCD キューのクリーンアップを待つ（十分な時間を確保）
        std::thread::sleep(Duration::from_millis(500));

        log::info!("Syphon Server を解放します");
        drop(syphon_server);
        log::info!("Syphon Server を解放しました");

        // 4. RenderContext を明示的に破棄（GL コンテキストが有効な状態で）
        log::info!("RenderContext を破棄します");
        drop(render_ctx);

        // 5. GL リソースを削除
        log::info!("GL リソースを削除します");
        gl::DeleteFramebuffers(1, &fbo);
        gl::DeleteTextures(1, &texture);
        // プレビュー用リソースも解放
        gl::DeleteFramebuffers(1, &preview_fbo);
        gl::DeleteTextures(1, &preview_texture);

        // 6. GL コンテキストを破棄
        // 注意: mpv インスタンスは MpvContext が管理しているので、ここでは破棄しない
        log::info!("GL コンテキストを破棄します");
        CGLDestroyContext(gl_ctx);
    }

    log::info!("Syphon レンダリングを終了しました");
    Ok(())
}

/// CGL コンテキストを作成
fn create_cgl_context() -> Result<CGLContextObj> {
    unsafe {
        let attributes = [
            CGL_PFA_ACCELERATED,
            CGL_PFA_OPENGL_PROFILE,
            CGL_OGL_VERSION_3_2_CORE,
            0,
        ];

        let mut pix_fmt: CGLPixelFormatObj = std::ptr::null_mut();
        let mut num_pix_fmts: i32 = 0;

        let status = CGLChoosePixelFormat(attributes.as_ptr(), &mut pix_fmt, &mut num_pix_fmts);

        if status != CGL_NO_ERROR {
            return Err(anyhow::anyhow!("CGLChoosePixelFormat に失敗: {}", status));
        }

        let mut ctx: CGLContextObj = std::ptr::null_mut();
        let status = CGLCreateContext(pix_fmt, std::ptr::null_mut(), &mut ctx);
        CGLDestroyPixelFormat(pix_fmt);

        if status != CGL_NO_ERROR {
            return Err(anyhow::anyhow!("CGLCreateContext に失敗: {}", status));
        }

        CGLSetCurrentContext(ctx);

        // GL 関数ポインタをロード（dlsym を使用）
        gl::load_with(|name| {
            let name_cstr = std::ffi::CString::new(name).unwrap();
            dlsym(RTLD_DEFAULT, name_cstr.as_ptr())
        });

        Ok(ctx)
    }
}

/// FBO とテクスチャを作成
fn create_fbo(width: u32, height: u32) -> (gl::types::GLuint, gl::types::GLuint) {
    let mut fbo: gl::types::GLuint = 0;
    let mut texture: gl::types::GLuint = 0;

    unsafe {
        // テクスチャを作成
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

        // FBO を作成してテクスチャをアタッチ
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

/// Syphon Server を作成
fn create_syphon_server(name: &str, gl_context: CGLContextObj) -> Result<Retained<AnyObject>> {
    // Syphon.framework を明示的にロード
    load_syphon_framework()?;

    unsafe {
        // SyphonServer クラスを取得
        let syphon_class = objc2::class!(SyphonServer);

        // サーバー名を NSString に変換
        let name_ns = NSString::from_str(name);

        // SyphonServer を初期化
        // [[SyphonServer alloc] initWithName:name context:cglContext options:nil]
        let alloc_ptr: *mut AnyObject = msg_send![syphon_class, alloc];
        let server_ptr: *mut AnyObject = msg_send![
            alloc_ptr,
            initWithName: &*name_ns,
            context: gl_context,
            options: std::ptr::null::<AnyObject>()
        ];
        let server = Retained::from_raw(server_ptr).ok_or_else(|| anyhow::anyhow!("Syphon Server の作成に失敗"))?;

        log::info!("Syphon Server を作成: {}", name);

        Ok(server)
    }
}

/// Syphon にフレームを公開
fn publish_syphon_frame(
    server: &Retained<AnyObject>,
    texture_id: gl::types::GLuint,
    width: u32,
    height: u32,
) {
    static FIRST_PUBLISH: std::sync::Once = std::sync::Once::new();

    unsafe {
        // NSSize を作成
        let size = NSSize {
            width: width as f64,
            height: height as f64,
        };

        FIRST_PUBLISH.call_once(|| {
            println!("First Syphon publish: texture_id={}, size={}x{}", texture_id, width, height);
            log::info!("Syphon 初回送信: texture_id={}, 解像度={}x{}", texture_id, width, height);
        });

        // publishFrameTexture:textureTarget:imageRegion:textureDimensions:flipped:
        let _: () = msg_send![
            &**server,
            publishFrameTexture: texture_id as u32,
            textureTarget: gl::TEXTURE_2D as u32,
            imageRegion: NSRect { origin: NSPoint { x: 0.0, y: 0.0 }, size },
            textureDimensions: size,
            flipped: false
        ];
    }
}

// NSSize, NSPoint, NSRect の定義
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct NSSize {
    width: f64,
    height: f64,
}

unsafe impl Encode for NSSize {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct NSPoint {
    x: f64,
    y: f64,
}

unsafe impl Encode for NSPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

unsafe impl Encode for NSRect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[NSPoint::ENCODING, NSSize::ENCODING]);
}

/// プレビューフレームを WebView に送信（glBlitFramebuffer で GPU リサイズ）
/// preview_fbo / preview_texture はループ外で確保済みのものを再利用する
unsafe fn send_preview_frame_blit(
    app: &tauri::AppHandle,
    src_fbo: gl::types::GLuint,
    width: u32,
    height: u32,
    preview_fbo: gl::types::GLuint,
    preview_texture: gl::types::GLuint,
) {
    let preview_width = 320u32;
    let preview_height = ((height as f32 / width as f32) * preview_width as f32).max(1.0) as u32;

    // glBlitFramebuffer で GPU 上でリサイズコピー（FBO・テクスチャは再利用）
    gl::BindFramebuffer(gl::READ_FRAMEBUFFER, src_fbo);
    gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, preview_fbo);
    gl::BlitFramebuffer(
        0, 0, width as _, height as _,
        0, 0, preview_width as _, preview_height as _,
        gl::COLOR_BUFFER_BIT,
        gl::LINEAR,
    );

    // 縮小したピクセルデータを読み取る
    gl::BindFramebuffer(gl::FRAMEBUFFER, preview_fbo);
    let mut pixels = vec![0u8; (preview_width * preview_height * 3) as usize];
    gl::ReadPixels(
        0, 0,
        preview_width as i32,
        preview_height as i32,
        gl::RGB,
        gl::UNSIGNED_BYTE,
        pixels.as_mut_ptr() as *mut _,
    );

    // GL エラーチェック
    let gl_error = gl::GetError();
    if gl_error != gl::NO_ERROR {
        log::warn!("プレビューフレーム読み取り時の GL エラー: 0x{:X}", gl_error);
        return;
    }

    // base64 エンコード
    use base64::Engine;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&pixels);

    // Tauri Event で送信
    #[derive(serde::Serialize, Clone)]
    struct PreviewFrame {
        width: u32,
        height: u32,
        data: String,
    }

    let _ = app.emit(
        "preview-frame",
        PreviewFrame {
            width: preview_width,
            height: preview_height,
            data: base64_data,
        },
    );
}

/// ダミー関数（output/mod.rs の send_texture から呼ばれる）
#[allow(dead_code)]
pub fn send(_texture_id: u32, _width: u32, _height: u32) {
    // この関数は Phase 3 完了後に実装する
    log::warn!("syphon::send() は未実装です");
}
