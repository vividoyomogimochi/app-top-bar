# LED AppBar

Windows のデスクトップ上端に常駐する LED ニュースティッカーウィジェット。
画面の作業領域を予約するため、他のウィンドウを最大化してもバーの下に収まる。

## 機能

- **画面上端に常駐表示** — 枠なし・最前面・タスクバー非表示
- **作業領域の予約** — Windows AppBar API (`SHAppBarNotify`) でタスクバーと同じ仕組みで画面端を確保
- **マルチモニタ対応** — トレイメニューからモニタを切り替え可能（設定は保存される）
- **バー高さ変更** — 40 / 60 / 80 / 100 / 120px から選択
- **自動起動** — Windows 起動時に自動実行（レジストリ Run キー）
- **システムトレイ** — モニタ選択・高さ変更・自動起動・終了

## 表示コンテンツ

[https://ticker.samoyed.moe/ticker/](https://ticker.samoyed.moe/ticker/) を WebView2 で表示する。
Canvas ベースの LED ドットマトリクス風ニュースティッカー。

## 必要環境

- Windows 10 / 11
- WebView2 Runtime（Windows 10/11 ならプリインストール済み）

## ビルド

```powershell
# 前提: Node.js, Rust, Visual Studio Build Tools (C++) が必要
npm install
npm run tauri dev    # 開発モード
npm run tauri build  # リリースビルド（MSI/NSIS生成）
```

## 設定ファイル

`%APPDATA%/app-top-bar/config.json` に保存される。

```json
{
  "bar_height": 80,
  "monitor": 0,
  "auto_start": true,
  "url": "https://ticker.samoyed.moe/ticker/"
}
```

## ライセンス

MIT
