/// mpv インスタンスの管理
///
/// ## 依存関係
/// - システムに `libmpv` がインストールされている必要があります
///   - macOS: `brew install mpv`
///   - Windows: libmpv-dev を PATH に追加
///
/// ## OpenGL レンダリングの仕組み
/// 1. `MpvContext::new()` が mpv インスタンスを初期化する
/// 2. `mpv_handle_ptr()` で mpv 内部ポインタを取得する
/// 3. preview.rs の render_loop がそのポインタを使って RenderContext を構築する
///    （GL コンテキストが current になった後に行う必要がある）
/// 4. プレビュースレッドが RenderContext を受け取り、毎フレーム FBO に描画する
use anyhow::Result;
use libmpv2::Mpv;

/// libmpv2::Error は Rc を内包するため Send+Sync でない。
/// map_err で文字列に変換して anyhow::Error に乗せるヘルパー。
fn mpv_err(e: libmpv2::Error) -> anyhow::Error {
    anyhow::anyhow!("mpv エラー: {:?}", e)
}

/// mpv インスタンスのラッパー
pub struct MpvContext {
    pub mpv: Mpv,
}

impl MpvContext {
    /// mpv を初期化する（再生は開始しない）
    pub fn new(url: &str, quality: Option<&str>) -> Result<Self> {
        let mpv = Mpv::new().map_err(mpv_err)?;

        // yt-dlp 連携を有効化（mpv が内蔵で呼び出す）
        mpv.set_property("ytdl", true).map_err(mpv_err)?;

        // 画質設定（デフォルト: best）
        let format = match quality {
            Some("1080p") => "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
            Some("720p")  => "bestvideo[height<=720]+bestaudio/best[height<=720]",
            Some("480p")  => "bestvideo[height<=480]+bestaudio/best[height<=480]",
            _             => "bestvideo+bestaudio/best",
        };
        mpv.set_property("ytdl-format", format).map_err(mpv_err)?;

        // ハードウェアアクセラレーション（可能なら使用）
        mpv.set_property("hwdec", "auto-safe").map_err(mpv_err)?;

        // キャッシュとバッファリング設定（カクツキ対策）
        mpv.set_property("cache", true).map_err(mpv_err)?;
        mpv.set_property("cache-secs", 10i64).map_err(mpv_err)?;
        mpv.set_property("demuxer-max-bytes", "150M").map_err(mpv_err)?;
        mpv.set_property("demuxer-max-back-bytes", "75M").map_err(mpv_err)?;
        mpv.set_property("cache-pause-initial", true).map_err(mpv_err)?;
        mpv.set_property("cache-pause-wait", 3i64).map_err(mpv_err)?;

        // NOTE: vo は設定しない（Syphon スレッドで RenderContext API を使用するため）

        // NOTE: loadfile は Syphon スレッドで RenderContext 作成後に実行する
        // URL は player/mod.rs 側で保持し、Syphon に渡す

        Ok(Self { mpv })
    }

    /// ファイルを読み込んで再生を開始する
    pub fn load_file(&self, url: &str) -> Result<()> {
        self.mpv.command("loadfile", &[url, "replace"]).map_err(mpv_err)?;
        Ok(())
    }

    /// mpv 内部ハンドルへの生ポインタを返す
    ///
    /// このポインタは preview.rs の render_loop で RenderContext を作成するために使う。
    /// ポインタは MpvContext のライフタイム内でのみ有効。
    pub fn mpv_handle_ptr(&self) -> *mut libmpv2_sys::mpv_handle {
        self.mpv.ctx.as_ptr()
    }

    /// 一時停止 / 再開トグル
    /// 戻り値: true = 一時停止中, false = 再生中
    pub fn toggle_pause(&self) -> Result<bool> {
        let current: bool = self.mpv.get_property("pause").map_err(mpv_err)?;
        self.mpv.set_property("pause", !current).map_err(mpv_err)?;
        Ok(!current)
    }

    /// オーディオデバイス一覧を取得する
    /// 戻り値: (device_id, display_name) のリスト
    pub fn list_audio_devices(&self) -> Result<Vec<(String, String)>> {
        // TODO Phase 4: libmpv2 の Node プロパティを使って audio-device-list をパース
        Ok(vec![
            ("auto".to_string(), "デフォルト".to_string()),
        ])
    }

    /// 出力オーディオデバイスを切り替える
    pub fn set_audio_device(&self, device_id: &str) -> Result<()> {
        let id = if device_id.is_empty() { "auto" } else { device_id };
        self.mpv.set_property("audio-device", id).map_err(mpv_err)?;
        Ok(())
    }

    /// ボリューム設定（0–100）
    pub fn set_volume(&self, volume: u8) -> Result<()> {
        self.mpv.set_property("volume", volume as i64).map_err(mpv_err)?;
        Ok(())
    }
}
