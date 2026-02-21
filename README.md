# yt-spout-syphon-bridge

YouTube 動画を **yt-dlp** でストリーミング再生し、映像フレームを **Syphon** (macOS) / **Spout** (Windows) でリアルタイムに外部アプリへ転送するデスクトップアプリ。

TouchDesigner, Resolume, VDMX などの映像ツールと組み合わせて使うことを想定しています。

## スクリーンショット

![screenshot](docs/screenshot.png)

## 機能

- YouTube URL を入力するだけで再生開始
- **Syphon** (macOS) / **Spout** (Windows) でフレームをリアルタイム共有
- WebView 内インラインプレビュー（表示/非表示切り替え可）
- 再生・一時停止・停止・シーク・ループ・再生速度変更
- オーディオ出力デバイス選択（CoreAudio で起動時から列挙）
- ミュート・ボリューム調整
- Chrome クッキーを使った認証済みコンテンツの再生

## 動作環境

| 項目 | 要件 |
|------|------|
| macOS | 12 Monterey 以降（Apple Silicon / Intel） |
| Windows | 10 / 11（Spout 対応、未テスト） |
| libmpv | brew install mpv（macOS）/ libmpv-dev（Windows） |
| yt-dlp | mpv が内蔵で呼び出す（別途インストール不要） |

## 技術スタック

| レイヤー | 技術 |
|---------|------|
| フレームワーク | Tauri v2 |
| フロントエンド | React + TypeScript + Tailwind CSS |
| ビルドツール | Vite |
| 動画デコード | libmpv2 crate（yt-dlp は mpv が内部で呼び出し） |
| GPU 共有 (mac) | Syphon Framework → Rust FFI (objc2 crate) |
| GPU 共有 (Win) | Spout2 SDK → Rust FFI (bindgen) |
| オーディオ列挙 | CoreAudio FFI（macOS）|

## セットアップ

### macOS

```bash
# 依存ライブラリ
brew install mpv pkg-config

# Node 依存関係
pnpm install

# 開発サーバー起動
pnpm tauri dev

# プロダクションビルド
pnpm tauri build
```

### Syphon.framework の準備

```bash
# Syphon フレームワークを以下に配置
src-tauri/bindings/syphon/Syphon.framework
```

[Syphon Framework](https://github.com/Syphon/Syphon-Framework) から取得してください。

### Windows（未テスト）

1. [libmpv-dev](https://sourceforge.net/projects/mpv-player-windows/files/libmpv/) をダウンロードして PATH を通す
2. [Spout2 SDK](https://github.com/leadedge/Spout2) の `SpoutLibrary.h` と `Spout2.lib` を `src-tauri/bindings/spout2/` に配置
3. `pnpm tauri dev`

## 使い方

1. アプリを起動
2. YouTube URL を入力して Enter または再生ボタンをクリック
3. TouchDesigner / Resolume 等で **Syphon In TOP** または **Spout In TOP** を追加
4. サーバー名 `yt-spout-syphon-bridge` を選択

## 受信側の設定例

### TouchDesigner

- **Syphon In TOP**（macOS）または **Spout In TOP**（Windows）を追加
- Server Name: `yt-spout-syphon-bridge`

### Resolume / VDMX

- Syphon / Spout 入力ソースから `yt-spout-syphon-bridge` を選択

## ライセンス

MIT
