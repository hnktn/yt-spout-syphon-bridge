# yt-spout-syphon-bridge

## プロジェクト概要

YouTube動画を `yt-dlp` でストリーミング再生し、映像フレームを **Spout** (Windows) / **Syphon** (macOS) で外部アプリ（TouchDesigner, Resolume, VDMX等）にリアルタイム転送するデスクトップアプリ。

## 技術スタック

| レイヤー | 技術 |
|---------|------|
| フレームワーク | Tauri v2 |
| フロントエンド | React + TypeScript + Tailwind CSS |
| ビルドツール | Vite |
| 動画デコード | libmpv (libmpv2 crate) — yt-dlp は mpv が内蔵で呼び出す |
| GPU共有 (Win) | Spout2 SDK → Rust FFI (bindgen) |
| GPU共有 (mac) | Syphon Framework → Rust FFI (objc2 crate) |
| プレビュー | glutin + OpenGL (Tauri WebView とは別ネイティブウィンドウ) |
| オーディオ | mpv の audio-device API (仮想デバイス選択対応) |

## アーキテクチャ

```
[React UI (WebView)]
  ↕ Tauri IPC
[Rust Backend]
  → libmpv + yt-dlp → OpenGL FBO (テクスチャ)
                              ↓              ↓
                    [Preview Window]   [Spout/Syphon Sender]
  → mpv audio pipeline → 選択された出力デバイス
```

## データフロー

1. ユーザーが YouTube URL を入力 → `play` IPC コマンド → Rust
2. `MpvContext::new(url)` で libmpv を初期化、yt-dlp 経由でストリーム取得・デコード開始
3. `mpv_render_context_render()` で OpenGL FBO に毎フレーム描画
4. FBO テクスチャを 2 系統に渡す:
   - **プレビュー**: glutin ウィンドウ内でテクスチャをフルスクリーン描画
   - **Spout/Syphon**: `SendTexture()` / `publishFrameTexture()` でテクスチャ ID を共有
5. 音声は mpv の `audio-device` プロパティで出力先を制御

## ディレクトリ構成

```
yt-spout-syphon-bridge/
├── CLAUDE.md                      ← このファイル
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/                           ← React フロントエンド
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   │   ├── UrlInput.tsx           # URL 入力フォーム
│   │   ├── TransportControls.tsx  # 再生/停止/一時停止
│   │   └── AudioDeviceSelector.tsx # 出力デバイス選択
│   └── hooks/
│       └── usePlayer.ts           # Tauri IPC ラッパー
└── src-tauri/                     ← Rust バックエンド
    ├── Cargo.toml
    ├── build.rs                   # bindgen (Spout2 FFI 生成)
    ├── tauri.conf.json
    ├── bindings/
    │   ├── spout2/                # Spout2 SDK headers + .lib (Windows)
    │   └── syphon/                # Syphon Framework headers (macOS)
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── commands.rs            # Tauri IPC コマンド定義
        ├── player/
        │   ├── mod.rs             # PlayerState (Tauri 管理状態)
        │   ├── mpv_context.rs     # libmpv 初期化 + render API
        │   └── audio.rs           # オーディオデバイス列挙
        └── output/
            ├── mod.rs             # プラットフォーム分岐
            ├── preview.rs         # glutin プレビューウィンドウ
            ├── spout.rs           # Spout2 FFI ラッパー (Windows)
            └── syphon.rs          # Syphon FFI ラッパー (macOS)
```

## 実装ステップ

### Phase 1: プロジェクトセットアップ + mpv 基本再生

```bash
# 依存ライブラリのインストール
# macOS
brew install mpv pkg-config

# Windows (管理者権限で)
# https://sourceforge.net/projects/mpv-player-windows/files/libmpv/
# libmpv-dev をダウンロードして PATH を通す

# Node 依存関係
pnpm install

# 開発サーバー起動
pnpm tauri dev
```

**実装内容:**
- `MpvContext::new(url, quality)` を実装
  - `Mpv::new()` で mpv インスタンス作成
  - `mpv.set_property("ytdl", true)` — yt-dlp 連携を有効化
  - `mpv.set_property("ytdl-format", "bestvideo+bestaudio/best")` — 画質設定
  - `mpv.command("loadfile", &[url])` — 再生開始
- Tauri IPC: `play`, `stop`, `pause`, `get_status`

### Phase 2: OpenGL レンダリングパイプライン

**実装内容:**
- `glutin` + `winit` でネイティブウィンドウを別スレッドで作成
- `mpv_render_context_create()` で OpenGL レンダコンテキストを初期化
  ```rust
  // OpenGL コールバック例
  let render_ctx = mpv.create_render_context(
      RenderContextType::OpenGL(OpenGLInitParams {
          get_proc_address: |name| gl_context.get_proc_address(name) as _,
      })
  )?;
  ```
- レンダリングループ:
  1. FBO 作成 (`gl::GenFramebuffers`)
  2. `render_ctx.render(FBO_ID, WIDTH, HEIGHT, true)` で mpv フレーム描画
  3. テクスチャをプレビューウィンドウに表示
  4. テクスチャを Spout/Syphon に渡す

### Phase 3: Spout/Syphon 出力

#### Windows (Spout2)

1. [Spout2 SDK](https://github.com/leadedge/Spout2) をダウンロード
2. `bindings/spout2/` に `SpoutLibrary.h` と `Spout2.lib` を配置
3. `build.rs` の bindgen が自動的に `spout_bindings.rs` を生成する
4. `output/spout.rs` を実装:
   ```rust
   // 疑似コード
   let spout = unsafe { bindings::GetSpout() };
   unsafe { (*spout).CreateSender("yt-bridge\0".as_ptr(), width, height) };
   // 毎フレーム
   unsafe { (*spout).SendTexture(texture_id, GL_TEXTURE_2D, width, height, false, fbo_id) };
   ```

#### macOS (Syphon)

1. Xcode で Syphon.framework をビルド
2. `output/syphon.rs` を `objc2` で実装:
   ```rust
   // 疑似コード (objc2 使用)
   let server = SyphonServer::new("yt-bridge", gl_context);
   // 毎フレーム
   server.publish_frame_texture(texture_id, GL_TEXTURE_RECTANGLE, size, ...);
   ```

### Phase 4: オーディオデバイス選択

- `mpv.get_property::<Vec<AudioDevice>>("audio-device-list")` でデバイス列挙
- `mpv.set_property("audio-device", device_id)` で切り替え
- 仮想デバイス例: Windows → VB-Cable, macOS → BlackHole
- React 側: `<AudioDeviceSelector>` でドロップダウン表示

### Phase 5: UI / エラーハンドリング / テスト

- Tauri イベント (`emit`) でリアルタイムステータス更新
- エラー状態 (無効URL, ネットワーク切断) のUI表示
- TouchDesigner の `Spout In TOP` / `Syphon In TOP` で受信テスト

## Tauri IPC コマンド仕様

| コマンド | 引数 | 戻り値 | 説明 |
|---------|------|--------|------|
| `play` | `{ url: string, quality?: string }` | `StatusResponse` | 再生開始 |
| `stop` | — | `StatusResponse` | 停止 |
| `pause` | — | `StatusResponse` | 一時停止トグル |
| `get_status` | — | `StatusResponse` | 現在状態取得 |
| `get_audio_devices` | — | `AudioDevice[]` | デバイス一覧 |
| `set_audio_device` | `{ device_id: string }` | `void` | デバイス切替 |
| `set_volume` | `{ volume: number }` | `void` | 音量設定 (0-100) |

```typescript
// StatusResponse 型定義
interface StatusResponse {
  status: 'idle' | 'loading' | 'playing' | 'paused' | 'error';
  url?: string;
  error?: string;
  spout_active: boolean;
  syphon_active: boolean;
}
```

## 主要 Cargo 依存関係

```toml
[dependencies]
tauri = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
log = "0.4"
env_logger = "0.11"
libmpv2 = "4"        # libmpv Rust bindings
glutin = "0.32"       # OpenGL コンテキスト
glutin-winit = "0.5"
winit = "0.30"
gl = "0.14"
once_cell = "1"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-foundation = { version = "0.2", features = ["NSString"] }
```

## 注意点 (Rust 初心者向け)

1. **OpenGL はシングルスレッド制約**: GL コンテキストを作ったスレッドでのみ GL 呼び出し可能。レンダリング専用スレッドを立て、`mpsc::channel` で Tauri backend と通信する。

2. **unsafe の扱い**: Spout/Syphon の FFI 呼び出しは `unsafe {}` が必要。安全なラッパー関数で囲むことで、呼び出し側は `unsafe` を意識しなくて済む。

3. **エラーハンドリング**: `anyhow::Result<T>` と `?` 演算子で統一。Tauri コマンドは `Result<T, String>` を返す (`anyhow::Error` は `.to_string()` で変換)。

4. **条件コンパイル**:
   ```rust
   #[cfg(target_os = "windows")]
   mod spout;

   #[cfg(target_os = "macos")]
   mod syphon;
   ```

5. **libmpv2 の注意**: システムに `libmpv` が必要。`PKG_CONFIG_PATH` が通っていないとビルドエラーになる。

## 言語規約

- コード中のコメントは**日本語**で記述する
- コミットメッセージは**日本語**で記述する
- ドキュメント・README 等は**日本語**で記述する
- Claude とのやりとり（質問・回答・説明）はすべて**日本語**でおこなう

## 参考リンク

- [Tauri v2 公式ドキュメント](https://v2.tauri.app/)
- [libmpv2 crate](https://crates.io/crates/libmpv2)
- [mpv render API ドキュメント](https://mpv.io/manual/master/#embedding-into-other-software-libmpv)
- [Spout2 SDK](https://github.com/leadedge/Spout2)
- [Syphon Framework](https://github.com/Syphon/Syphon-Framework)
- [glutin 0.32 examples](https://github.com/rust-windowing/glutin/tree/master/glutin_examples)
- [objc2 crate](https://docs.rs/objc2/latest/objc2/)
