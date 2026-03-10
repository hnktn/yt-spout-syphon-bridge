/// NDI 出力モジュール
///
/// NDI SDK (libndi) を動的にロードし、映像フレームをネットワーク経由で送信する。
/// Spout/Syphon と異なり、GPU テクスチャ共有ではなく CPU ベースのピクセルデータ送信。
///
/// ## 前提条件
/// - NDI Tools がインストールされていること（libndi.dylib / ndi.dll がシステムに存在）
/// - NDI SDK のヘッダは不要（FFI 定義は手動で記述）

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ─── NDI SDK の FFI 定義 ──────────────────────────────────────────────────────

/// NDI 送信インスタンス（不透明ポインタ）
#[repr(C)]
pub struct NDIlib_send_instance_t {
    _opaque: [u8; 0],
}

/// NDI 送信設定
#[repr(C)]
pub struct NDIlib_send_create_t {
    /// ソース名（UTF-8 null 終端）
    pub p_ndi_name: *const std::ffi::c_char,
    /// グループ名（NULL でデフォルト）
    pub p_groups: *const std::ffi::c_char,
    /// クロック参照を使用するか
    pub clock_video: bool,
    /// クロック参照を使用するか（オーディオ）
    pub clock_audio: bool,
}

/// NDI ビデオフレーム v2
#[repr(C)]
pub struct NDIlib_video_frame_v2_t {
    /// 幅（ピクセル）
    pub xres: i32,
    /// 高さ（ピクセル）
    pub yres: i32,
    /// FourCC フォーマット
    pub four_cc: u32,
    /// フレームレート（分子）
    pub frame_rate_n: i32,
    /// フレームレート（分母）
    pub frame_rate_d: i32,
    /// アスペクト比（0.0 でデフォルト）
    pub picture_aspect_ratio: f32,
    /// フレームフォーマット（プログレッシブ / インターレース）
    pub frame_format_type: u32,
    /// タイムコード（100ns 単位、INT64_MAX で自動）
    pub timecode: i64,
    /// ピクセルデータポインタ
    pub p_data: *const u8,
    /// 1行あたりのバイト数
    pub line_stride_in_bytes: i32,
    /// メタデータ XML（NULL で省略）
    pub p_metadata: *const std::ffi::c_char,
    /// タイムスタンプ（0 で自動）
    pub timestamp: i64,
}

/// FourCC: BGRA (NDI のネイティブフォーマットの一つ)
const NDIFOURCC_BGRA: u32 = u32::from_le_bytes([b'B', b'G', b'R', b'A']);
/// FourCC: RGBA
const NDIFOURCC_RGBA: u32 = u32::from_le_bytes([b'R', b'G', b'B', b'A']);

/// フレームフォーマット: プログレッシブ
const NDILIB_FRAME_FORMAT_TYPE_PROGRESSIVE: u32 = 1;

/// タイムコード自動設定
const NDILIB_SEND_TIMECODE_SYNTHESIZE: i64 = i64::MAX;

// ─── 動的ロード用の関数ポインタ型 ──────────────────────────────────────────────

type FnInitialize = unsafe extern "C" fn() -> bool;
type FnDestroy = unsafe extern "C" fn();
type FnSendCreate = unsafe extern "C" fn(p_create_settings: *const NDIlib_send_create_t) -> *mut NDIlib_send_instance_t;
type FnSendDestroy = unsafe extern "C" fn(p_instance: *mut NDIlib_send_instance_t);
type FnSendVideoV2 = unsafe extern "C" fn(p_instance: *mut NDIlib_send_instance_t, p_video_data: *const NDIlib_video_frame_v2_t);

// ─── NDI ライブラリラッパー ────────────────────────────────────────────────────

/// NDI ライブラリの動的ロード済みハンドル
struct NdiLib {
    _lib: libloading::Library,
    initialize: FnInitialize,
    destroy: FnDestroy,
    send_create: FnSendCreate,
    send_destroy: FnSendDestroy,
    send_video_v2: FnSendVideoV2,
}

// NdiLib は内部でスレッドセーフな C ライブラリのポインタを保持
unsafe impl Send for NdiLib {}
unsafe impl Sync for NdiLib {}

impl NdiLib {
    /// libndi を動的にロードする
    fn load() -> anyhow::Result<Self> {
        // プラットフォームに応じたライブラリパスを試行
        let lib_paths: Vec<&str> = if cfg!(target_os = "macos") {
            vec![
                "libndi.dylib",
                "/usr/local/lib/libndi.dylib",
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                "Processing.NDI.Lib.x64.dll",
                "ndi.dll",
            ]
        } else {
            vec![
                "libndi.so",
                "libndi.so.5",
            ]
        };

        let mut last_error = None;
        for path in &lib_paths {
            match unsafe { libloading::Library::new(path) } {
                Ok(lib) => {
                    log::info!("NDI ライブラリをロードしました: {}", path);
                    return Self::from_lib(lib);
                }
                Err(e) => {
                    log::debug!("NDI ライブラリのロードに失敗 ({}): {}", path, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow::anyhow!(
            "NDI ライブラリが見つかりません。NDI Tools をインストールしてください。最後のエラー: {:?}",
            last_error
        ))
    }

    fn from_lib(lib: libloading::Library) -> anyhow::Result<Self> {
        unsafe {
            let initialize: FnInitialize = *lib.get(b"NDIlib_initialize\0")
                .map_err(|e| anyhow::anyhow!("NDIlib_initialize が見つかりません: {}", e))?;
            let destroy: FnDestroy = *lib.get(b"NDIlib_destroy\0")
                .map_err(|e| anyhow::anyhow!("NDIlib_destroy が見つかりません: {}", e))?;
            let send_create: FnSendCreate = *lib.get(b"NDIlib_send_create_v2\0")
                // v2 がなければ v1 を試す
                .or_else(|_| lib.get(b"NDIlib_send_create\0"))
                .map_err(|e| anyhow::anyhow!("NDIlib_send_create が見つかりません: {}", e))?;
            let send_destroy: FnSendDestroy = *lib.get(b"NDIlib_send_destroy\0")
                .map_err(|e| anyhow::anyhow!("NDIlib_send_destroy が見つかりません: {}", e))?;
            let send_video_v2: FnSendVideoV2 = *lib.get(b"NDIlib_send_send_video_v2\0")
                .map_err(|e| anyhow::anyhow!("NDIlib_send_send_video_v2 が見つかりません: {}", e))?;

            Ok(Self {
                _lib: lib,
                initialize,
                destroy,
                send_create,
                send_destroy,
                send_video_v2,
            })
        }
    }
}

// ─── NDI 送信ハンドル ─────────────────────────────────────────────────────────

/// NDI 送信インスタンスのラッパー
pub struct NdiSender {
    lib: Arc<NdiLib>,
    instance: *mut NDIlib_send_instance_t,
    /// NDI 送信が有効かどうか
    enabled: Arc<AtomicBool>,
}

unsafe impl Send for NdiSender {}

impl NdiSender {
    /// NDI 送信を初期化する
    pub fn new(source_name: &str) -> anyhow::Result<Self> {
        let lib = NdiLib::load()?;

        // NDI を初期化
        let ok = unsafe { (lib.initialize)() };
        if !ok {
            return Err(anyhow::anyhow!("NDI の初期化に失敗しました"));
        }
        log::info!("NDI を初期化しました");

        // 送信インスタンスを作成
        let name_cstr = std::ffi::CString::new(source_name)
            .map_err(|e| anyhow::anyhow!("ソース名が無効です: {}", e))?;

        let create_settings = NDIlib_send_create_t {
            p_ndi_name: name_cstr.as_ptr(),
            p_groups: std::ptr::null(),
            clock_video: true,
            clock_audio: false,
        };

        let instance = unsafe { (lib.send_create)(&create_settings) };
        if instance.is_null() {
            unsafe { (lib.destroy)() };
            return Err(anyhow::anyhow!("NDI 送信インスタンスの作成に失敗しました"));
        }

        log::info!("NDI 送信を開始: {}", source_name);

        Ok(Self {
            lib: Arc::new(lib),
            instance,
            enabled: Arc::new(AtomicBool::new(true)),
        })
    }

    /// NDI の有効/無効状態を共有するフラグを取得
    pub fn enabled_flag(&self) -> Arc<AtomicBool> {
        self.enabled.clone()
    }

    /// RGBA ピクセルデータを NDI フレームとして送信する
    ///
    /// # 引数
    /// * `data` - RGBA ピクセルデータ（上→下の行順序）
    /// * `width` - 幅（ピクセル）
    /// * `height` - 高さ（ピクセル）
    pub fn send_video_rgba(&self, data: &[u8], width: u32, height: u32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let expected_size = (width * height * 4) as usize;
        if data.len() < expected_size {
            log::warn!("NDI: ピクセルデータサイズが不足: {} < {}", data.len(), expected_size);
            return;
        }

        let video_frame = NDIlib_video_frame_v2_t {
            xres: width as i32,
            yres: height as i32,
            four_cc: NDIFOURCC_RGBA,
            frame_rate_n: 30000,
            frame_rate_d: 1001,
            picture_aspect_ratio: 0.0,
            frame_format_type: NDILIB_FRAME_FORMAT_TYPE_PROGRESSIVE,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: data.as_ptr(),
            line_stride_in_bytes: (width * 4) as i32,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };

        unsafe {
            (self.lib.send_video_v2)(self.instance, &video_frame);
        }
    }

    /// BGRA ピクセルデータを NDI フレームとして送信する
    pub fn send_video_bgra(&self, data: &[u8], width: u32, height: u32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let expected_size = (width * height * 4) as usize;
        if data.len() < expected_size {
            log::warn!("NDI: ピクセルデータサイズが不足: {} < {}", data.len(), expected_size);
            return;
        }

        let video_frame = NDIlib_video_frame_v2_t {
            xres: width as i32,
            yres: height as i32,
            four_cc: NDIFOURCC_BGRA,
            frame_rate_n: 30000,
            frame_rate_d: 1001,
            picture_aspect_ratio: 0.0,
            frame_format_type: NDILIB_FRAME_FORMAT_TYPE_PROGRESSIVE,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: data.as_ptr(),
            line_stride_in_bytes: (width * 4) as i32,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };

        unsafe {
            (self.lib.send_video_v2)(self.instance, &video_frame);
        }
    }
}

impl Drop for NdiSender {
    fn drop(&mut self) {
        log::info!("NDI 送信を停止します");
        unsafe {
            (self.lib.send_destroy)(self.instance);
            (self.lib.destroy)();
        }
        log::info!("NDI 送信を停止しました");
    }
}

// ─── グローバル NDI 状態管理 ──────────────────────────────────────────────────

use std::sync::Mutex;
use once_cell::sync::Lazy;

/// グローバル NDI 有効/無効フラグ
/// Syphon ループから参照される
static NDI_ENABLED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

/// NDI が利用可能かどうか（libndi がロード可能か）
static NDI_AVAILABLE: Lazy<Mutex<Option<bool>>> = Lazy::new(|| Mutex::new(None));

/// NDI の有効/無効を切り替える
pub fn set_enabled(enabled: bool) {
    NDI_ENABLED.store(enabled, Ordering::SeqCst);
    log::info!("NDI 出力を{}にしました", if enabled { "有効" } else { "無効" });
}

/// NDI が有効かどうかを返す
pub fn is_enabled() -> bool {
    NDI_ENABLED.load(Ordering::SeqCst)
}

/// NDI が利用可能か（libndi が存在するか）を確認する
pub fn is_available() -> bool {
    let mut cached = NDI_AVAILABLE.lock().unwrap();
    if let Some(available) = *cached {
        return available;
    }

    // 初回チェック: libndi をロードできるか試す
    let available = NdiLib::load().is_ok();
    *cached = Some(available);

    if available {
        log::info!("NDI ライブラリが利用可能です");
    } else {
        log::info!("NDI ライブラリが見つかりません（NDI 出力は無効）");
    }

    available
}

/// Syphon ループ内で使用する NDI 送信関数
///
/// FBO からピクセルデータを読み取り、NDI フレームとして送信する。
/// GL コンテキストが有効な状態で呼び出す必要がある。
///
/// # 引数
/// * `sender` - NDI 送信インスタンス
/// * `fbo` - 読み取り元の FBO ID
/// * `width` - 幅（ピクセル）
/// * `height` - 高さ（ピクセル）
/// * `pixel_buffer` - 再利用するピクセルバッファ（アロケーション回避のため）
pub fn send_frame_from_fbo(
    sender: &NdiSender,
    fbo: u32,
    width: u32,
    height: u32,
    pixel_buffer: &mut Vec<u8>,
) {
    let required_size = (width * height * 4) as usize;
    pixel_buffer.resize(required_size, 0);

    unsafe {
        // FBO からピクセルデータを読み取る
        gl::BindFramebuffer(gl::READ_FRAMEBUFFER, fbo);
        gl::ReadPixels(
            0, 0,
            width as i32,
            height as i32,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            pixel_buffer.as_mut_ptr() as *mut _,
        );

        let gl_error = gl::GetError();
        if gl_error != gl::NO_ERROR {
            log::warn!("NDI: glReadPixels エラー: 0x{:X}", gl_error);
            return;
        }
    }

    // OpenGL は左下原点なので上下反転が必要
    flip_vertical_rgba(pixel_buffer, width, height);

    // NDI フレームを送信
    sender.send_video_rgba(pixel_buffer, width, height);
}

/// RGBA ピクセルデータを上下反転する（OpenGL 座標系 → 画像座標系）
fn flip_vertical_rgba(data: &mut [u8], width: u32, height: u32) {
    let stride = (width * 4) as usize;
    let half_height = height as usize / 2;
    for y in 0..half_height {
        let top = y * stride;
        let bottom = (height as usize - 1 - y) * stride;
        // 行をスワップ
        for x in 0..stride {
            data.swap(top + x, bottom + x);
        }
    }
}
