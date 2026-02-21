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
    /// mpv を初期化する（loadfile は実行しない）
    /// 注意: RenderContext を作成してから load_file() を呼ぶ必要がある
    pub fn new(_url: &str, quality: Option<&str>) -> Result<Self> {
        let mpv = Mpv::new().map_err(mpv_err)?;

        // yt-dlp 連携を有効化（mpv が内蔵で呼び出す）
        mpv.set_property("ytdl", true).map_err(mpv_err)?;

        // Chrome クッキーを使用
        mpv.set_property("ytdl-raw-options", "cookies-from-browser=chrome").map_err(mpv_err)?;

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

        // RenderContext API を使用するため vo=libmpv を設定
        mpv.set_property("vo", "libmpv").map_err(mpv_err)?;

        // キャッシュとバッファリング設定（カクツキ対策）
        mpv.set_property("cache", true).map_err(mpv_err)?;
        mpv.set_property("cache-secs", 10i64).map_err(mpv_err)?;
        mpv.set_property("demuxer-max-bytes", "150M").map_err(mpv_err)?;
        mpv.set_property("demuxer-max-back-bytes", "75M").map_err(mpv_err)?;
        mpv.set_property("cache-pause-initial", true).map_err(mpv_err)?;
        mpv.set_property("cache-pause-wait", 3i64).map_err(mpv_err)?;

        // 音声ピッチ補正を有効化（速度変更時に音程を保持）
        mpv.set_property("audio-pitch-correction", true).map_err(mpv_err)?;

        // 注意: loadfile は RenderContext 作成後に Syphon スレッドで実行する

        Ok(Self { mpv })
    }

    /// URL をロードして再生を開始する
    /// 注意: RenderContext 作成後に呼ぶ必要がある
    pub fn load_file(&self, url: &str) -> Result<()> {
        self.mpv.command("loadfile", &[url, "replace"]).map_err(mpv_err)?;
        log::info!("loadfile コマンドを実行: {}", url);
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
        use libmpv2::mpv_node::MpvNode;

        // audio-device-list プロパティを取得
        let node: MpvNode = self.mpv.get_property("audio-device-list").map_err(mpv_err)?;
        log::debug!("audio-device-list ノード取得成功");

        let mut devices = Vec::new();

        // ノードを配列として解析
        if let Some(array) = node.array() {
            log::debug!("配列として解析成功");
            for (i, item) in array.enumerate() {
                log::debug!("配列要素 {}: {:?}", i, item);
                if let Some(map) = item.map() {
                    log::debug!("  マップとして解析成功");
                    let mut name = String::new();
                    let mut description = String::new();

                    // マップから name と description を取得
                    for (key, value) in map {
                        log::debug!("    キー: {}", key);
                        match key.as_str() {
                            "name" => {
                                if let Some(s) = value.str() {
                                    name = s.to_string();
                                    log::debug!("      name = {}", name);
                                }
                            }
                            "description" => {
                                if let Some(s) = value.str() {
                                    description = s.to_string();
                                    log::debug!("      description = {}", description);
                                }
                            }
                            _ => {}
                        }
                    }

                    if !name.is_empty() {
                        let display_name = if !description.is_empty() {
                            description
                        } else {
                            name.clone()
                        };
                        log::debug!("  デバイス追加: {} ({})", name, display_name);
                        devices.push((name, display_name));
                    }
                } else {
                    log::warn!("配列要素 {} はマップではありません", i);
                }
            }
        } else {
            log::warn!("audio-device-list は配列ではありません");
        }

        // 少なくとも auto デバイスは返す
        if devices.is_empty() {
            log::warn!("デバイスが1つも見つかりませんでした。auto を追加します");
            devices.push(("auto".to_string(), "デフォルト".to_string()));
        }

        log::info!("オーディオデバイス一覧を取得: {} 個", devices.len());
        for (id, name) in &devices {
            log::info!("  - {} ({})", id, name);
        }

        Ok(devices)
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

    /// ミュート設定
    pub fn set_mute(&self, mute: bool) -> Result<()> {
        self.mpv.set_property("mute", mute).map_err(mpv_err)?;
        Ok(())
    }

    /// ミュート状態を取得
    pub fn get_mute(&self) -> Result<bool> {
        match self.mpv.get_property("mute") {
            Ok(mute) => Ok(mute),
            Err(e) => {
                log::warn!("mute 取得失敗: {:?}", e);
                Ok(false)
            }
        }
    }

    /// ループ再生の設定
    pub fn set_loop(&self, enabled: bool) -> Result<()> {
        let value = if enabled { "inf" } else { "no" };
        self.mpv.set_property("loop-file", value).map_err(mpv_err)?;
        Ok(())
    }

    /// ループ再生の状態を取得
    pub fn get_loop(&self) -> Result<bool> {
        match self.mpv.get_property::<String>("loop-file") {
            Ok(value) => Ok(value == "inf"),
            Err(e) => {
                log::warn!("loop-file 取得失敗: {:?}", e);
                Ok(false)
            }
        }
    }

    /// シーク（秒単位）
    pub fn seek(&self, seconds: f64) -> Result<()> {
        self.mpv.command("seek", &[&seconds.to_string(), "absolute"]).map_err(mpv_err)?;
        Ok(())
    }

    /// 再生位置を取得（秒）
    pub fn get_time_pos(&self) -> Result<f64> {
        match self.mpv.get_property("time-pos") {
            Ok(pos) => Ok(pos),
            Err(e) => {
                // 再生停止中などで取得できない場合は 0.0 を返す
                log::debug!("time-pos 取得失敗（通常動作の可能性あり）: {:?}", e);
                Ok(0.0)
            }
        }
    }

    /// 総再生時間を取得（秒）
    pub fn get_duration(&self) -> Result<f64> {
        match self.mpv.get_property("duration") {
            Ok(dur) => Ok(dur),
            Err(e) => {
                // ライブストリームなどで取得できない場合は 0.0 を返す
                log::debug!("duration 取得失敗（通常動作の可能性あり）: {:?}", e);
                Ok(0.0)
            }
        }
    }

    /// 再生速度を設定（0.25 〜 4.0）
    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let clamped = speed.clamp(0.25, 4.0);
        log::info!("再生速度を設定: {}", clamped);
        self.mpv.set_property("speed", clamped).map_err(mpv_err)?;

        // 設定後の値を確認
        let actual: f64 = self.mpv.get_property("speed").map_err(mpv_err)?;
        log::info!("設定後の再生速度: {}", actual);

        Ok(())
    }

    /// 再生速度を取得
    pub fn get_speed(&self) -> Result<f64> {
        match self.mpv.get_property("speed") {
            Ok(speed) => Ok(speed),
            Err(e) => {
                log::warn!("speed 取得失敗: {:?}", e);
                Ok(1.0)
            }
        }
    }

    /// 動画タイトルを取得
    pub fn get_media_title(&self) -> Result<String> {
        // media-title プロパティを試す（YouTube などのメタデータ）
        if let Ok(title) = self.mpv.get_property::<String>("media-title") {
            if !title.is_empty() {
                return Ok(title);
            }
        }

        // フォールバック: filename プロパティ
        match self.mpv.get_property("filename") {
            Ok(filename) => Ok(filename),
            Err(e) => {
                log::warn!("filename 取得失敗: {:?}", e);
                Ok(String::from("(タイトル不明)"))
            }
        }
    }
}
