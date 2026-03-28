# LED AppBar

デスクトップの上端に常駐する、LED ドットマトリクス風のニュースティッカーです。
Windows の AppBar API を使って画面端の領域を確保するので、ウィンドウを最大化してもバーと重なりません。タスクバーと同じ仕組みです。

表示するコンテンツは [ticker.samoyed.moe](https://ticker.samoyed.moe/ticker/) から取得し、WebView2 で Canvas ベースの LED 風アニメーションとして描画します。

## 主な機能

- 画面上端に枠なし・最前面で常駐（タスクバーには表示されません）
- マルチモニタ対応 — トレイメニューから表示先を切り替えられます
- バーの高さを 40〜120px の 5 段階で変更できます
- フルスクリーンアプリ起動時に自動で非表示になります
- サーバプロセスの同時起動 — SSE/WebSocket 配信サーバなどを指定でき、アプリと共に起動・終了します（クラッシュ時は自動リスタート）
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
  "url": "https://ticker.samoyed.moe/ticker/",
  "auto_hide_fullscreen": true,
  "server_command": null
}
```

## デフォルトウィジェット — LED News Ticker

デフォルトで表示されるコンテンツは [led-news-ticker](https://github.com/vividoyomogimochi/led-news-ticker) です。
ブラウザベースの LED ドットマトリクス風ニュースティッカーで、RSS や WebSocket からフィードを受け取ってスクロール表示します。

- ビルド時に約 22,340 グリフのビットマップフォントアトラスを生成し、ブラウザ間で一貫したドットパターンを実現しています
- テーマを切り替えて背景や配色をカスタマイズできます
- 設定画面（`/config`）からソース（コンテンツ）とディスプレイ（見た目）を個別に選択できます

`config.json` の `url` を変更すれば、任意の Web ページをウィジェットとして表示することも可能です。

## ライセンス

MIT
