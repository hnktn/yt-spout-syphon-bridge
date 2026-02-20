/// GPU テクスチャ共有出力モジュール
///
/// プラットフォームに応じて Spout (Windows) / Syphon (macOS) を切り替える。
/// 全実装は Phase 3 で行う。

#[cfg(target_os = "windows")]
pub mod spout;

#[cfg(target_os = "macos")]
pub mod syphon;

// macOS では OpenGL ベースのプレビューを使用（Phase 3 では無効化）
#[cfg(target_os = "macos")]
pub mod preview;

// Windows では OpenGL ベースのプレビューを使用
#[cfg(target_os = "windows")]
pub mod preview;

/// OpenGL テクスチャを Spout/Syphon に送信する共通インターフェース
/// Phase 3 で実装する
#[allow(dead_code)]
pub fn send_texture(texture_id: u32, width: u32, height: u32) {
    #[cfg(target_os = "windows")]
    spout::send(texture_id, width, height);

    #[cfg(target_os = "macos")]
    syphon::send(texture_id, width, height);
}
