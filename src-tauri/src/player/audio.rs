/// mpv が起動していない状態でもデバイス一覧を返すフォールバック
///
/// Phase 4 で mpv の audio-device-list プロパティに置き換える
pub fn enumerate_devices() -> Vec<(String, String)> {
    // プラットフォーム別のデフォルトデバイス候補
    // 実際の実装では OS のオーディオ API を使う

    #[cfg(target_os = "windows")]
    {
        // WASAPI デバイス一覧 (Windows Audio Session API)
        // TODO: windows-rs または cpal クレートで列挙する
        vec![
            ("auto".to_string(), "デフォルト".to_string()),
            // VB-Cable などの仮想デバイスも自動で表示されるはず
        ]
    }

    #[cfg(target_os = "macos")]
    {
        // CoreAudio デバイス一覧
        // TODO: coreaudio-rs クレートで列挙する
        vec![
            ("auto".to_string(), "デフォルト".to_string()),
            // BlackHole, Loopback などが表示されるはず
        ]
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        vec![("auto".to_string(), "デフォルト".to_string())]
    }
}
