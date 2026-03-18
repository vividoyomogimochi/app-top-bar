# LED AppBar

デスクトップの上端に常駐する、LED ドットマトリクス風のニュースティッカーです。
Windows の AppBar API を使って画面端の領域を確保するので、ウィンドウを最大化してもバーと重なりません。タスクバーと同じ仕組みです。

表示するコンテンツは [ticker.samoyed.moe](https://ticker.samoyed.moe/ticker/) から取得し、WebView2 で Canvas ベースの LED 風アニメーションとして描画します。

## 主な機能

- 画面上端に枠なし・最前面で常駐（タスクバーには表示されません）
- マルチモニタ対応 — トレイメニューから表示先を切り替えられます
- バーの高さを 40〜120px の 5 段階で変更できます
- Windows 起動時の自動実行に対応しています（レジストリ Run キー）
- すべての設定はシステムトレイのメニューから操作できます

## 必要な環境

- Windows 10 または 11
- WebView2 Runtime（通常はプリインストール済み）

## ビルド方法

Node.js、Rust、Visual Studio Build Tools (C++) が必要です。

```powershell
npm install
npm run tauri dev    # 開発モードで起動
npm run tauri build  # リリースビルド（MSI / NSIS インストーラを生成）
```

## 設定ファイル

設定は `%APPDATA%/app-top-bar/config.json` に保存されます。

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
