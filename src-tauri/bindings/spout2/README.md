# Spout2 SDK (Windows 用)

このディレクトリに Spout2 SDK のファイルを配置してください。

## ダウンロード先

https://github.com/leadedge/Spout2

## 配置するファイル

```
bindings/spout2/
├── SpoutLibrary.h      ← C API ヘッダ (必須)
└── SpoutLibrary.lib    ← スタティックリンクライブラリ (必須)
```

## 取得手順

1. 上記リポジトリをクローン
2. `SPOUTSDK/SpoutLibrary/` フォルダ内のヘッダをコピー
3. ビルド済みの `.lib` は `Binaries/x64/` にあります

これらのファイルが揃っていれば `cargo build` 時に `build.rs` が自動でRust FFIバインディングを生成します。

ファイルがない場合は Spout 出力機能が無効化されます（ビルドは通ります）。
