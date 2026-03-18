# アーキテクチャ

## 全体構成

```
LED AppBar (Tauri v2)
├── Rust バックエンド (src-tauri/src/)
│   ├── main.rs        エントリポイント
│   ├── lib.rs         アプリ初期化・ウィンドウ作成・プラグイン登録
│   ├── appbar.rs      Windows AppBar API ラッパー
│   ├── config.rs      設定の読み書き (JSON)
│   └── tray.rs        システムトレイメニュー
└── フロントエンド (src/)
    └── index.html     フォールバック用（通常は外部URL直接読み込み）
```

## コア技術

### AppBar API (`appbar.rs`)

Windows Shell の `SHAppBarMessage` を使って画面端の作業領域を予約する。
タスクバーと同じ仕組みで、他のウィンドウの最大化範囲がバーを避けるようになる。

```
起動時の流れ:
  ABM_NEW       → AppBar として登録
  ABM_QUERYPOS  → 指定エッジで利用可能な位置を問い合わせ
  ABM_SETPOS    → 位置を確定（作業領域が更新される）
  MoveWindow    → 実際のウィンドウを移動
  SetWindowPos  → HWND_TOPMOST で最前面を保証

終了時:
  ABM_REMOVE    → 登録解除（作業領域が元に戻る）
```

`windows-rs` クレート (v0.61) の `Win32::UI::Shell` を使用。

### モニタ列挙

`EnumDisplayMonitors` で全モニタを取得し、左座標→上座標でソートして
Windows ディスプレイ設定の並び順と一致させている。

### ウィンドウ作成 (`lib.rs`)

`tauri.conf.json` の宣言的ウィンドウではなく、`WebviewWindowBuilder` で
プログラマティックに作成している。理由:

- `initialization_script` でスクロールバー非表示の CSS を注入できる
- `shadow(false)` でウィンドウ周囲の隙間を除去できる
- 設定ファイルの URL を動的に読み込める

### 設定 (`config.rs`)

`%APPDATA%/app-top-bar/config.json` に JSON で永続化。
`dirs` クレートでプラットフォームごとの設定ディレクトリを解決。

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| `bar_height` | u32 | 80 | バーの高さ (px) |
| `monitor` | u32 | 0 | 表示モニタのインデックス |
| `auto_start` | bool | true | Windows 起動時に自動実行 |
| `url` | String | ticker URL | WebView に読み込む URL |

### トレイメニュー (`tray.rs`)

`CheckMenuItem` を使ったラジオボタン風のメニュー。
アイテムの参照を `TrayMenuItems` 構造体で保持し、選択時に
`set_checked()` で状態を更新する（メニュー再構築は ID 衝突を起こすため不可）。

### 自動起動

`tauri-plugin-autostart` を使用。内部的には `auto-launch` クレートが
`HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` に exe パスを登録する。

設定変更時は即座に `enable()` / `disable()` を呼んでレジストリと同期。

## 依存関係

| クレート | 用途 |
|---------|------|
| `tauri` v2 | アプリフレームワーク、WebView2 統合 |
| `windows` v0.61 | Win32 AppBar API 呼び出し |
| `tauri-plugin-autostart` | Windows 自動起動 (レジストリ) |
| `serde` / `serde_json` | 設定ファイルのシリアライズ |
| `dirs` | OS 標準の設定ディレクトリ取得 |

## 既知の制限

- タスクバーがあるモニタで初回起動時に 1 行分ズレることがある（トレイからサイズ変更で解消）
- AppBar は 1 モニタにつきエッジ 1 つしか登録できない（上端にタスクバーがある場合は競合する）
