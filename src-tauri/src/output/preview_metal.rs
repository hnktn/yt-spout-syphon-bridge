/// Metal ベースのプレビューモジュール（macOS 専用）
///
/// ## 実装方針
/// 1. mpv は OpenGL でレンダリング（libmpv の render API は OpenGL ベース）
/// 2. OpenGL テクスチャを IOSurface 経由で Metal と共有
/// 3. Metal テクスチャからピクセルデータを読み取り
/// 4. base64 エンコードして Tauri Event で WebView に送信（15fps）

use anyhow::Result;
use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id, ClassType};
use objc2_foundation::{NSArray, NSString};
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLTexture};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

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

#[link(name = "OpenGL", kind = "framework")]
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
    fn CGLGetProcAddress(name: *const std::ffi::c_char) -> *const std::ffi::c_void;

    fn CGLTexImageIOSurface2D(
        ctx: CGLContextObj,
        target: gl::types::GLenum,
        internal_format: gl::types::GLint,
        width: gl::types::GLsizei,
        height: gl::types::GLsizei,
        format: gl::types::GLenum,
        ty: gl::types::GLenum,
        iosurface: *mut AnyObject,
        plane: u32,
    ) -> CGLError;
}

#[allow(non_camel_case_types)]
type CFDictionaryRef = *const std::ffi::c_void;

#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    fn IOSurfaceCreate(properties: CFDictionaryRef) -> *mut AnyObject;
}

// MTLRegion, MTLOrigin, MTLSize の定義
#[repr(C)]
#[allow(dead_code)]
struct MTLOrigin {
    x: u64,
    y: u64,
    z: u64,
}

#[repr(C)]
#[allow(dead_code)]
struct MTLSize {
    width: u64,
    height: u64,
    depth: u64,
}

#[repr(C)]
#[allow(dead_code)]
struct MTLRegion {
    origin: MTLOrigin,
    size: MTLSize,
}

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

/// Metal ベースのプレビューレンダリングを別スレッドで起動する
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
        if let Err(e) = render_loop_metal(sendable, app_handle, cmd_rx, width, height) {
            log::error!("Metal レンダリングループでエラー: {}", e);
        }
    });

    Ok(PreviewHandle { cmd_tx })
}

/// Metal + OpenGL ハイブリッドレンダリングループ
///
/// mpv (OpenGL) → FBO → IOSurface → Metal Texture → CPU メモリ → base64 → Tauri Event
fn render_loop_metal(
    mpv_handle: SendableMpvHandle,
    app_handle: AppHandle,
    cmd_rx: mpsc::Receiver<RenderCommand>,
    width: u32,
    height: u32,
) -> Result<()> {
    use objc2::ffi::NSUInteger;

    // Metal デバイスを取得
    let device = unsafe { MTLCreateSystemDefaultDevice() }
        .ok_or_else(|| anyhow::anyhow!("Metal デバイスの作成に失敗"))?;

    log::info!("Metal デバイス: {:?}", device.name());

    // CGL (Core OpenGL) コンテキストを作成
    // macOS では OpenGL と Metal を IOSurface で連携させる
    let (gl_ctx, fbo, texture, iosurface) = create_gl_context_with_iosurface(width, height)?;

    // mpv の RenderContext を作成
    let render_ctx = unsafe {
        // OpenGL コンテキストを current にする
        let _ = CGLSetCurrentContext(gl_ctx);

        // get_proc_address 用のクロージャ
        fn get_proc_addr(_ctx: &*const std::ffi::c_void, name: &str) -> *mut std::ffi::c_void {
            let name_cstr = std::ffi::CString::new(name).unwrap();
            unsafe {
                let sym = CGLGetProcAddress(name_cstr.as_ptr());
                sym as *mut std::ffi::c_void
            }
        }

        let ctx_ptr = &gl_ctx as *const _ as *const std::ffi::c_void;
        RenderContext::new(
            &mut *mpv_handle.0,
            [
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address: get_proc_addr,
                    ctx: ctx_ptr,
                }),
            ],
        )
        .map_err(|e| anyhow::anyhow!("RenderContext の作成に失敗: {:?}", e))?
    };

    log::info!("Metal + OpenGL ハイブリッドレンダリング開始: {}x{}", width, height);

    // IOSurface から Metal テクスチャを作成
    let metal_texture = create_metal_texture_from_iosurface(&device, iosurface, width, height)?;

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

        unsafe {
            // OpenGL コンテキストを current にする
            let _ = CGLSetCurrentContext(gl_ctx);

            // mpv に FBO へ描画させる
            if let Err(e) = render_ctx.render::<()>(fbo as i32, width as i32, height as i32, true) {
                log::warn!("mpv render エラー: {:?}", e);
                std::thread::sleep(Duration::from_millis(16));
                continue;
            }

            // OpenGL から IOSurface へフラッシュ
            gl::Flush();
        }

        // 一定間隔で Metal テクスチャからピクセルデータを読み取って WebView に送信
        if last_emit.elapsed() >= frame_interval {
            // Metal テクスチャから CPU メモリにコピー
            read_metal_texture_to_cpu(&metal_texture, &mut pixels, width, height)?;

            // Tauri Event で WebView に送信（base64 エンコード）
            let b64 = base64_encode_pixels(&pixels);
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
        CGLDestroyContext(gl_ctx);
    }

    log::info!("Metal レンダリングを終了しました");
    Ok(())
}

/// CGL コンテキストと IOSurface 共有 FBO を作成
fn create_gl_context_with_iosurface(
    width: u32,
    height: u32,
) -> Result<(
    CGLContextObj,
    gl::types::GLuint,
    gl::types::GLuint,
    *mut AnyObject,
)> {
    unsafe {
        // CGL ピクセルフォーマットを作成
        let attributes = [
            CGL_PFA_ACCELERATED,
            CGL_PFA_OPENGL_PROFILE,
            CGL_OGL_VERSION_3_2_CORE,
            0,
        ];

        let mut pix_fmt: CGLPixelFormatObj = std::ptr::null_mut();
        let mut num_pix_fmts: i32 = 0;

        let status = CGLChoosePixelFormat(
            attributes.as_ptr(),
            &mut pix_fmt,
            &mut num_pix_fmts,
        );

        if status != CGL_NO_ERROR {
            return Err(anyhow::anyhow!("CGLChoosePixelFormat に失敗: {}", status));
        }

        // CGL コンテキストを作成
        let mut ctx: CGLContextObj = std::ptr::null_mut();
        let status = CGLCreateContext(pix_fmt, std::ptr::null_mut(), &mut ctx);
        CGLDestroyPixelFormat(pix_fmt);

        if status != CGL_NO_ERROR {
            return Err(anyhow::anyhow!("CGLCreateContext に失敗: {}", status));
        }

        // コンテキストを current にする
        CGLSetCurrentContext(ctx);

        // GL 関数ポインタをロード
        gl::load_with(|name| {
            let name_cstr = std::ffi::CString::new(name).unwrap();
            CGLGetProcAddress(name_cstr.as_ptr()) as *const _
        });

        // IOSurface を作成
        let iosurface = create_iosurface(width, height)?;

        // IOSurface をバックエンドとする OpenGL テクスチャを作成
        let mut texture: gl::types::GLuint = 0;
        gl::GenTextures(1, &mut texture);
        gl::BindTexture(gl::TEXTURE_RECTANGLE_ARB, texture);

        // IOSurface をテクスチャにバインド
        let status = CGLTexImageIOSurface2D(
            ctx,
            gl::TEXTURE_RECTANGLE_ARB,
            gl::RGBA as _,
            width as _,
            height as _,
            gl::BGRA,
            gl::UNSIGNED_INT_8_8_8_8_REV,
            iosurface,
            0,
        );

        if status != CGL_NO_ERROR {
            return Err(anyhow::anyhow!(
                "CGLTexImageIOSurface2D に失敗: {}",
                status
            ));
        }

        // FBO を作成してテクスチャをアタッチ
        let mut fbo: gl::types::GLuint = 0;
        gl::GenFramebuffers(1, &mut fbo);
        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
        gl::FramebufferTexture2D(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_RECTANGLE_ARB,
            texture,
            0,
        );

        let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
        if status != gl::FRAMEBUFFER_COMPLETE {
            return Err(anyhow::anyhow!("FBO が不完全: 0x{:X}", status));
        }

        gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

        Ok((ctx, fbo, texture, iosurface))
    }
}

/// IOSurface を作成（OpenGL ↔ Metal 共有用）
fn create_iosurface(width: u32, height: u32) -> Result<*mut AnyObject> {
    unsafe {
        use objc2::ffi::CFDictionaryRef;
        use objc2::runtime::NSObject;

        // IOSurfaceCreate の引数となる辞書を作成
        let dict_class = objc2::class!(NSMutableDictionary);
        let dict: *mut AnyObject = msg_send_id![dict_class, new].as_ptr() as *mut AnyObject;

        // Width
        let width_key = NSString::from_str("IOSurfaceWidth");
        let width_num: *mut AnyObject = msg_send_id![
            objc2::class!(NSNumber),
            numberWithUnsignedInt: width
        ]
        .as_ptr() as *mut AnyObject;
        let _: () = msg_send![dict, setObject: width_num, forKey: &*width_key];

        // Height
        let height_key = NSString::from_str("IOSurfaceHeight");
        let height_num: *mut AnyObject = msg_send_id![
            objc2::class!(NSNumber),
            numberWithUnsignedInt: height
        ]
        .as_ptr() as *mut AnyObject;
        let _: () = msg_send![dict, setObject: height_num, forKey: &*height_key];

        // BytesPerElement (RGBA = 4)
        let bpe_key = NSString::from_str("IOSurfaceBytesPerElement");
        let bpe_num: *mut AnyObject =
            msg_send_id![objc2::class!(NSNumber), numberWithUnsignedInt: 4u32].as_ptr()
                as *mut AnyObject;
        let _: () = msg_send![dict, setObject: bpe_num, forKey: &*bpe_key];

        // PixelFormat (BGRA = 'BGRA')
        let fmt_key = NSString::from_str("IOSurfacePixelFormat");
        let fmt_num: *mut AnyObject = msg_send_id![
            objc2::class!(NSNumber),
            numberWithUnsignedInt: u32::from_be_bytes(*b"BGRA")
        ]
        .as_ptr() as *mut AnyObject;
        let _: () = msg_send![dict, setObject: fmt_num, forKey: &*fmt_key];

        // IOSurfaceCreate を呼び出し
        let iosurface: *mut AnyObject = objc2::ffi::IOSurfaceCreate(dict as CFDictionaryRef);

        if iosurface.is_null() {
            return Err(anyhow::anyhow!("IOSurface の作成に失敗"));
        }

        Ok(iosurface)
    }
}

/// IOSurface から Metal テクスチャを作成
fn create_metal_texture_from_iosurface(
    device: &Retained<objc2_metal::MTLDevice>,
    iosurface: *mut AnyObject,
    width: u32,
    height: u32,
) -> Result<Retained<MTLTexture>> {
    unsafe {
        use objc2_metal::MTLPixelFormat;

        // MTLTextureDescriptor を作成
        let desc_class = objc2::class!(MTLTextureDescriptor);
        let desc: *mut AnyObject = msg_send_id![desc_class, new].as_ptr() as *mut AnyObject;

        let _: () = msg_send![desc, setTextureType: 2u64]; // MTLTextureType2D = 2
        let _: () = msg_send![desc, setPixelFormat: MTLPixelFormat::BGRA8Unorm as u64];
        let _: () = msg_send![desc, setWidth: width as u64];
        let _: () = msg_send![desc, setHeight: height as u64];
        let _: () = msg_send![desc, setUsage: 1u64]; // MTLTextureUsageShaderRead = 1

        // IOSurface から Metal テクスチャを作成
        let texture: *mut objc2_metal::MTLTexture = msg_send![
            device.as_ref() as *const _ as *mut AnyObject,
            newTextureWithDescriptor: desc,
            iosurface: iosurface,
            plane: 0u64
        ];

        if texture.is_null() {
            return Err(anyhow::anyhow!("Metal テクスチャの作成に失敗"));
        }

        Ok(Retained::from_raw(texture).unwrap())
    }
}

/// Metal テクスチャから CPU メモリにピクセルデータを読み取る
fn read_metal_texture_to_cpu(
    texture: &Retained<MTLTexture>,
    pixels: &mut [u8],
    width: u32,
    height: u32,
) -> Result<()> {
    unsafe {
        // MTLRegion を作成
        let region = MTLRegion {
            origin: MTLOrigin {
                x: 0,
                y: 0,
                z: 0,
            },
            size: MTLSize {
                width: width as u64,
                height: height as u64,
                depth: 1,
            },
        };

        // getBytes でピクセルデータを読み取る
        let bytes_per_row = (width * 4) as u64;
        let _: () = msg_send![
            texture.as_ref() as *const _ as *mut AnyObject,
            getBytes: pixels.as_mut_ptr(),
            bytesPerRow: bytes_per_row,
            fromRegion: region,
            mipmapLevel: 0u64
        ];
    }

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
