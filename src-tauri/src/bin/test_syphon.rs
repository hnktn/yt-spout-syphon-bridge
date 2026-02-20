/// Syphon framework のロードと YouTube 再生をテストする簡易プログラム
///
/// 使用方法:
/// DYLD_FRAMEWORK_PATH=./bindings/syphon cargo run --bin test_syphon

use anyhow::Result;

fn main() -> Result<()> {
    env_logger::init();

    log::info!("=== Syphon Framework テスト開始 ===");

    // YouTube URL
    let url = "https://www.youtube.com/watch?v=C-CYwNz3z8w";
    let server_name = "yt-spout-syphon-test";
    let width = 1280u32;
    let height = 720u32;

    log::info!("YouTube URL: {}", url);
    log::info!("Syphon サーバー名: {}", server_name);
    log::info!("解像度: {}x{}", width, height);

    // Syphon スレッドを起動
    log::info!("Syphon 出力を起動します...");

    // mpv_handle は NULL で開始（Syphon スレッド内で作成する）
    // テストプログラムなので app_handle は None
    let handle = app_lib::output::syphon::spawn(
        std::ptr::null_mut(),
        server_name,
        url,
        width,
        height,
        None, // プレビュー無効
    )?;

    log::info!("Syphon スレッドが起動しました");
    log::info!("TouchDesigner や VDMX で Syphon サーバー '{}' を探してください", server_name);
    log::info!("Ctrl+C で終了します");

    // メインスレッドは待機（Ctrl+C まで）
    std::thread::park();

    // クリーンアップ
    handle.stop();
    log::info!("Syphon 出力を停止しました");

    Ok(())
}
