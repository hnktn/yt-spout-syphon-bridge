/// Spout2 出力モジュール (Windows 専用)
///
/// ## 事前準備
/// 1. https://github.com/leadedge/Spout2 から SDK をダウンロード
/// 2. `bindings/spout2/` に以下を配置:
///    - `SpoutLibrary.h`  (C API ヘッダ)
///    - `SpoutLibrary.lib` (リンクライブラリ)
/// 3. `cargo build` すると build.rs が自動で FFI バインディングを生成する
///
/// ## 動作原理
/// Spout2 は DirectX / OpenGL のテクスチャを共有メモリ経由で渡す。
/// mpv が描画した OpenGL テクスチャ ID を `SendTexture()` に渡すだけでよい。

// Phase 3 で実装する
// build.rs が生成した spout_bindings.rs を include する
// include!(concat!(env!("OUT_DIR"), "/spout_bindings.rs"));

use std::sync::OnceLock;

/// Spout Sender の初期化状態
static SENDER_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Sender を初期化する (アプリ起動時に 1 回だけ呼ぶ)
#[allow(dead_code)]
pub fn init(width: u32, height: u32) {
    SENDER_INITIALIZED.get_or_init(|| {
        log::info!("Spout2 sender init: {}x{}", width, height);
        // TODO Phase 3:
        // unsafe {
        //     let spout = bindings::GetSpout();
        //     (*spout).CreateSender(
        //         b"yt-spout-syphon-bridge\0".as_ptr() as *const i8,
        //         width,
        //         height,
        //     );
        // }
        true
    });
}

/// OpenGL テクスチャを Spout 経由で送信する
#[allow(dead_code)]
pub fn send(texture_id: u32, width: u32, height: u32) {
    // TODO Phase 3:
    // unsafe {
    //     let spout = bindings::GetSpout();
    //     (*spout).SendTexture(
    //         texture_id,
    //         gl::TEXTURE_2D,
    //         width,
    //         height,
    //         false,   // invert (false = normal orientation)
    //         0,       // FBO ID (0 = current FBO)
    //     );
    // }
    log::trace!("Spout::send texture={} {}x{}", texture_id, width, height);
}

/// Sender を解放する
#[allow(dead_code)]
pub fn release() {
    // TODO Phase 3:
    // unsafe {
    //     let spout = bindings::GetSpout();
    //     (*spout).ReleaseSender();
    //     (*spout).Release();
    // }
}
