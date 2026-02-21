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

    // mpv インスタンスを作成（テストプログラムでは MpvContext を使用せず、直接作成）
    log::info!("mpv インスタンスを作成します...");
    use libmpv2::Mpv;
    let mpv = Mpv::new().expect("mpv の作成に失敗");
    mpv.set_property("ytdl", true).expect("ytdl の設定に失敗");
    mpv.set_property("ytdl-raw-options", "cookies-from-browser=chrome").expect("ytdl-raw-options の設定に失敗");
    mpv.set_property("ytdl-format", "bestvideo+bestaudio/best").expect("ytdl-format の設定に失敗");
    mpv.set_property("hwdec", "auto-safe").expect("hwdec の設定に失敗");
    mpv.set_property("vo", "libmpv").expect("vo の設定に失敗");
    mpv.set_property("cache", true).expect("cache の設定に失敗");
    mpv.set_property("cache-secs", 10i64).expect("cache-secs の設定に失敗");
    // 注意: loadfile は Syphon スレッドで RenderContext 作成後に実行する

    let mpv_handle = mpv.ctx.as_ptr();

    // Syphon スレッドを起動
    log::info!("Syphon 出力を起動します...");

    // テストプログラムなので app_handle は None
    let handle = app_lib::output::syphon::spawn(
        mpv_handle,
        server_name,
        url,
        width,
        height,
        None, // プレビュー無効
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )?;

    log::info!("Syphon スレッドが起動しました");
    log::info!("TouchDesigner や VDMX で Syphon サーバー '{}' を探してください", server_name);
    log::info!("Ctrl+C で終了します");

    // メインスレッドは待機（Ctrl+C まで）
    // mpv インスタンスを保持し続ける必要がある
    std::thread::park();

    // クリーンアップ
    handle.stop();
    drop(mpv); // mpv を明示的に破棄
    log::info!("Syphon 出力を停止しました");

    Ok(())
}
