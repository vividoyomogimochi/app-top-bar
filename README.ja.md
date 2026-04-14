# LED AppBar

**デスクトップの上端に、LED ドットマトリクス風のスクロールティッカーを常駐させる Windows アプリです。**

LED AppBar は画面の最上部に LED ドットマトリクス風の表示を貼り付けます。ただの最前面ウィンドウではなく、`SHAppBarMessage` を使って自身を **本物の AppBar** として登録します。タスクバーと同じ仕組みなので、ウィンドウを最大化してもバーと重なりません。

表示される内容は WebView2 でレンダリングされる通常の Web ページです。既定では RSS/WebSocket ベースのニュースティッカー ([led-news-ticker](https://github.com/vividoyomogimochi/led-news-ticker)) を表示しますが、URL は設定で変更でき、任意の Web ページを表示できます。

**ダウンロード:** [BOOTH](https://vym.booth.pm/items/8192059) (¥300) / [itch.io](https://bunnyyears.itch.io/led-appbar) (pay-what-you-want)
**English README:** [README.md](README.md)

## 主な機能

- 本物の AppBar として登録 — ウィンドウを最大化してもバーと重ならない
- マルチモニタ対応（トレイメニューから表示先モニタを選択可能）
- バー高さを 40〜120 px の 5 段階で調整可能
- 他アプリのフルスクリーン時に自動で非表示 (`ABN_FULLSCREENAPP`)
- Windows 起動時の自動実行（レジストリ Run キー）
- 任意のサーバプロセスを同時起動 — SSE/WebSocket 配信サーバなどをアプリと共に起動・終了、クラッシュ時は自動リスタート
- 全ての設定はシステムトレイメニューから操作可能

## 必要な環境

- Windows 10 または 11
- WebView2 Runtime（通常はプリインストール済み）

## 動作の仕組み

バー本体は枠なしの Tauri (Rust) ウィンドウです。起動時に `ABM_NEW` に続いて `ABM_SETPOS` をシェルに送信し、選択されたモニタの上端に AppBar として登録します。シェルはその領域を予約するので、最大化されたウィンドウが自動的にバーの領域を避けて配置されます — タスクバーの挙動とまったく同じです。

ウィンドウの中身はローカル HTML ページで、WebView2 上にロードされます。そのページ内で、LED ドットマトリクスのアニメーションを約 22,000 グリフ（ASCII、JIS-X 0208、主要な記号を含む）の事前レンダリング済みビットマップフォントアトラスを使って `<canvas>` に描画しています。実行時フォントレンダリングではなくアトラス方式を採用することで、マシンやフォントレンダラ設定に依存しない一貫したドットパターンを実現しています。

フルスクリーン検知は AppBar の標準通知 `ABN_FULLSCREENAPP` を使います。同じモニタ上で他アプリがフルスクリーンになるとバーは自動的に非表示となり、フルスクリーンが終わると再び表示されます。

オプションで指定できる同時起動サーバプロセスはメインアプリが監視します。指定されたコマンドで起動し、stdout/stderr をキャプチャし、予期しない終了時は自動的に再起動します。ローカルなフィードサーバ（RSS 集約、WebSocket 中継など）をバーと一緒に実行したい場合に便利です。

## ビルド方法

Node.js、Rust、Visual Studio Build Tools (C++) が必要です。

```powershell
npm install
npm run tauri dev    # 開発モードで起動
npm run tauri build  # リリースビルド（MSI / NSIS インストーラを生成）
```

## 設定

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

全ての項目はトレイメニュー UI から編集できるので、通常はファイルを直接触る必要はありません。

## デフォルトウィジェット — LED News Ticker

デフォルトの表示内容は [led-news-ticker](https://github.com/vividoyomogimochi/led-news-ticker) です。ブラウザベースの LED ドットマトリクス風ニュースティッカーで、RSS フィードや WebSocket ストリームを取り込んでスクロール表示します。テーマや配色は内蔵の設定ページ (`/config`) からカスタマイズできます。

別のウィジェットを使いたい場合は、`config.json` の `url` 項目を変更するか、トレイメニューから編集してください。任意の Web ページが使えます。

## ライセンス

MIT. [LICENSE](LICENSE) を参照してください。
