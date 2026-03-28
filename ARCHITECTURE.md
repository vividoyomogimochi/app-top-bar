# アーキテクチャ

## 全体構成

```
LED AppBar (Tauri v2)
├── Rust バックエンド (src-tauri/src/)
│   ├── main.rs        エントリポイント
│   ├── lib.rs         アプリ初期化・ウィンドウ作成・プラグイン登録
│   ├── appbar.rs      Windows AppBar API ラッパー
│   ├── config.rs      設定の読み書き (JSON)
│   ├── server.rs      サーバプロセス管理（起動・監視・自動リスタート）
│   └── tray.rs        システムトレイメニュー
└── フロントエンド (src/)
    ├── index.html              フォールバック用（通常は外部URL直接読み込み）
    ├── settings.html           URL 設定ダイアログ
    └── server-settings.html    サーバ設定ダイアログ
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
  ↓
  Tauri イベントループが position(0,0) をワークエリア基準で再適用
  → WindowEvent::Moved で検知し、EXPECTED_RECT に基づいて SetWindowPos で補正

終了時:
  ABM_REMOVE    → 登録解除（作業領域が元に戻る）
```

#### ワークエリア再適用問題

`ABM_SETPOS` がワークエリアを変更した後、Tauri のイベントループが
`position(0,0)` を新しいワークエリア基準で再適用し、ウィンドウが
`bar_height` 分だけ下にズレる。Win32 API 側では正しい位置に配置
しているが、Tauri が内部的に保持する論理座標で上書きしてしまう。

対策として `EXPECTED_RECT`（`register_appbar` で確定した物理ピクセル座標）
を保持し、`WindowEvent::Moved` 発火時に `GetWindowRect` で現在位置と
比較、ズレていれば `SetWindowPos` で即座にスナップバックする。

この問題はタスクバーがあるモニタでのみ発生する。AppBar のスペース予約が
ワークエリアを変更するモニタでしか座標のズレが起きないため。

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
Tauri の `app.path().app_data_dir()` でプラットフォームごとの設定ディレクトリを解決。

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| `bar_height` | u32 | 80 | バーの高さ (px) |
| `monitor` | u32 | 0 | 表示モニタのインデックス |
| `auto_start` | bool | true | Windows 起動時に自動実行 |
| `url` | String | ticker URL | WebView に読み込む URL |
| `auto_hide_fullscreen` | bool | true | フルスクリーン時に自動非表示 |
| `server_command` | String? | null | アプリと共に起動するサーバの実行ファイルパス |

### トレイメニュー (`tray.rs`)

`CheckMenuItem` を使ったラジオボタン風のメニュー。
アイテムの参照を `TrayMenuItems` 構造体で保持し、選択時に
`set_checked()` で状態を更新する（メニュー再構築は ID 衝突を起こすため不可）。

### サーバプロセス管理 (`server.rs`)

トレイメニューの「Set Server...」から指定した実行ファイル（.exe / .bat / .cmd）を
アプリと共にバックグラウンドで起動する。SSE や WebSocket を配信するサーバプロセスの
同時起動を想定した機能。

```
起動の流れ (Windows):
  Command::new(path)
    .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
    .spawn()
  → Job Object を作成し JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE を設定
  → 子プロセスを Job に割り当て
  → サスペンドされたスレッドを ResumeThread で再開
```

**Job Object**: .bat が起動した node.exe のような孫プロセスもまとめて管理するため、
Windows の Job Object を使う。`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` フラグにより、
Job ハンドルを閉じた時点でプロセスツリー全体が終了する。アプリがクラッシュした場合も
OS が Job ハンドルを回収するため孤児プロセスは残らない。

**ウォッチドッグ**: バックグラウンドスレッドが `try_wait()` で 1 秒ごとにプロセスを
監視し、予期しない終了を検知すると 3 秒後に自動リスタートする。3 回連続で失敗した場合は
ダイアログで通知して諦める。

プロセスの終了タイミング:
- アプリ終了時（Quit / CloseRequested）
- サーバパスの変更・クリア時（旧プロセスを停止してから新プロセスを起動）

### 自動起動

`tauri-plugin-autostart` を使用。内部的には `auto-launch` クレートが
`HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` に exe パスを登録する。

設定変更時は即座に `enable()` / `disable()` を呼んでレジストリと同期。

## 依存関係

| クレート | 用途 |
|---------|------|
| `tauri` v2 | アプリフレームワーク、WebView2 統合 |
| `windows` v0.61 | Win32 AppBar / Job Object / ToolHelp API |
| `tauri-plugin-autostart` | Windows 自動起動 (レジストリ) |
| `tauri-plugin-single-instance` | 二重起動防止 |
| `tauri-plugin-dialog` | ファイル選択ダイアログ |
| `serde` / `serde_json` | 設定ファイルのシリアライズ |

## デバッグのヒント

ウィンドウ位置の問題を調査するとき、`appbar.rs` に以下のような診断コードを
一時的に仕込むと原因の切り分けがしやすい。

```rust
// GetWindowRect / GetClientRect をタイムスタンプ付きでファイルに書き出す
fn dump_diag(hwnd: HWND, label: &str, diag: &mut String) {
    unsafe {
        let mut wr: RECT = mem::zeroed();
        let _ = GetWindowRect(hwnd, &mut wr);
        let mut cr: RECT = mem::zeroed();
        let _ = GetClientRect(hwnd, &mut cr);
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        writeln!(diag,
            "[{label}] WindowRect=({},{},{},{}) ClientRect=({},{},{},{}) style=0x{:08X} ex=0x{:08X}",
            wr.left, wr.top, wr.right, wr.bottom,
            cr.left, cr.top, cr.right, cr.bottom,
            style, ex_style
        ).ok();
    }
}
```

**ポイント**: `setup()` 内で正しくても、イベントループ開始後に Tauri/WRY が
座標を上書きする場合がある。`std::thread::spawn` で 500ms〜2s 後に
`GetWindowRect` を再チェックすると、事後の変化を捕捉できる。

出力先は `%APPDATA%/app-top-bar/diag.log` が設定ディレクトリと同じで便利。

## 既知の制限

- AppBar は 1 モニタにつきエッジ 1 つしか登録できない（上端にタスクバーがある場合は競合する）
