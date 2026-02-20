# Syphon Framework (macOS 用)

このディレクトリに Syphon Framework を配置してください。

## ダウンロード先

https://github.com/Syphon/Syphon-Framework

## ビルドと配置

```bash
git clone https://github.com/Syphon/Syphon-Framework.git
cd Syphon-Framework
xcodebuild -configuration Release
# ビルド後に bindings/syphon/ にコピー
cp -r build/Release/Syphon.framework ../../bindings/syphon/
```

## 配置後の構成

```
bindings/syphon/
└── Syphon.framework/
    ├── Headers/
    │   ├── Syphon.h
    │   ├── SyphonServer.h
    │   └── SyphonClient.h
    └── Syphon  (バイナリ)
```

## tauri.conf.json への追記 (Phase 3)

```json
"bundle": {
  "macOS": {
    "frameworks": ["./bindings/syphon/Syphon.framework"]
  }
}
```
