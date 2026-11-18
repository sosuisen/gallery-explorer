# architecture

## アプリの全体像

GalleryExplorerは、お気に入り機能の付いたWindows用のファイルエクスプローラーである。
Rustで書き、画面は`windows-reactor`(WinUI 3の宣言的UIライブラリ、プレビュー版)で作る。

画面は1つのウィンドウだけである。

| 場所 | 内容 |
|---|---|
| 上 | 左から、お気に入りの追加と削除のボタン、「上へ」ボタン、パンくず |
| 左 | お気に入りの一覧、フォルダのツリー |
| 右 | 今のフォルダの中身の一覧 |
| 下 | 項目の数、またはエラーの文 |

## レイヤーとモジュール

依存の向きは、`ui` → `explorer` → `infra` → `domain`の一方向である。`domain`は他のどれにも依存しない。

| モジュール | 役割 | 主な型と関数 |
|---|---|---|
| `domain` | ファイルシステムも画面も使わないルール | `Entry`、`sort_entries`、`crumbs`、`resolve_crumb`、`display_name`、`Favorites`、`Tree`、`DoubleClick` |
| `infra` | ファイルシステムとお気に入りのファイルへのアクセス | `read_folder`、`child_folders`、`drives`、`FavoritesStore`、`open_with_default_app` |
| `explorer` | ウィンドウの状態と、ユーザーの操作 | `Explorer` |
| `ui` | 状態をWinUIのコントロールに変える | `ExplorerWindow`、`Message` |
| `main` | エントリーポイント | `main` |

`ui`と`main`、`infra/shell.rs`にはテストがない。WinUIのウィンドウや別のプログラムの起動が要るためである。
画面に関わる判断は、テストできるように`explorer`と`domain`に置く。

## 外部とのやり取り

- フォルダの読み込み: `std::fs::read_dir`を使う。
- ドライブの一覧: `A:\`から`Z:\`のうち、存在するものを並べる。
- ファイルを開く: `explorer.exe <パス>`を起動する。Windowsが既定のアプリで開く。
- お気に入りの保存: `%APPDATA%\GalleryExplorer\favorites.json`に、パスの文字列のJSON配列で保存する。
- 実行時には、Windows App Runtimeがインストールされている必要がある。

## 主な処理の流れ

### 起動

1. `main`が`App::run_component::<ExplorerWindow>`を呼ぶ。
2. `ExplorerWindow::create`が、`drives`でツリーのルートを得る。開始フォルダは`%USERPROFILE%`である。
3. `Explorer::open`が、`FavoritesStore::load`でお気に入りを読み、`read_folder`で開始フォルダを読む。
4. `ExplorerWindow::view`が、`Explorer`の状態から画面を作る。

### 操作

1. コントロールのイベントが`Message`になり、`ExplorerWindow::update`に届く。
2. `update`が、`Explorer`のメソッドを1つ呼ぶ。
3. `windows-reactor`が`view`を呼び直し、変わったコントロールだけを更新する。

### 一覧の行の選択と、ダブルクリックで開く

一覧は、`ScrollViewer`の中の`StackPanel`に、行ごとの`Border`を並べて作る。選択とダブルクリックは自前で扱う。

1. 一覧の行の`Border`が、左ボタンの押下のたびに`Message::RowPressed(行, 時刻)`を送る。
2. `Explorer::press_row`が、押された行を選択にする。選択された行は、灰色の背景と左のアクセント色の線で示す。
3. `Explorer::press_row`が`DoubleClick`に渡す。同じ行への500ms以内の2回目の押下が、ダブルクリックになる。
4. フォルダなら`navigate`する。ファイルならパスを返し、`ui`が`open_with_default_app`で開く。

## windows-reactor 0.100.0の制約と、その対応

- `TreeView`は、展開のイベントがなく、クリックではノードの文字しか分からない。そこで、ツリーは字下げしたボタンの行で作る。
- `BreadcrumbBar`は、クリックされたラベルしか通知しない。同じ名前のフォルダがパスに2つあるときは、深いほうの親へ移る。
- 一覧は`ListView`ではなく`StackPanel`で作る。理由は[ADR 001](adr/adr-001-stackpanel-for-file-list.md)にある。
- `StackPanel`は、項目を仮想化しない。項目がとても多いフォルダでは遅くなる。
- 一覧は、キーボードの矢印キーでは選べない。

## ADRの一覧

- [ADR 001: StackPanel for the File List](adr/adr-001-stackpanel-for-file-list.md)
