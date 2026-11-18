# architecture

## アプリの全体像

GalleryExplorerは、お気に入り機能の付いたWindows用のファイルエクスプローラーである。
Rustで書き、画面は`windows-reactor`(WinUI 3の宣言的UIライブラリ、プレビュー版)で作る。

画面は1つのウィンドウだけである。

| 場所 | 内容 |
|---|---|
| 上 | 左から、お気に入りの追加と削除のボタン、「上へ」ボタン、一覧とグリッドの切り替えボタン、パンくず |
| 左 | お気に入りの一覧、フォルダのツリー |
| 右 | 今のフォルダの中身。一覧か、サムネイルのグリッドで示す |
| 下 | 項目の数、またはエラーの文 |

## レイヤーとモジュール

依存の向きは、`ui` → `explorer` → `infra` → `domain`の一方向である。`domain`は他のどれにも依存しない。

| モジュール | 役割 | 主な型と関数 |
|---|---|---|
| `domain` | ファイルシステムも画面も使わないルール | `Entry`、`sort_entries`、`crumbs`、`resolve_crumb`、`display_name`、`Favorites`、`Tree`、`DoubleClick` |
| `infra` | ファイルシステムとお気に入りのファイルへのアクセス | `read_folder`、`child_folders`、`folder_covers`、`collect_thumbnails`、`shell_thumbnail`、`drives`、`FavoritesStore`、`open_with_default_app`、`FolderWatcher`、`Changes` |
| `explorer` | ウィンドウの状態と、ユーザーの操作 | `Explorer`、`ViewMode` |
| `ui` | 状態をWinUIのコントロールに変える | `ExplorerWindow`、`Message` |
| `main` | エントリーポイント | `main` |

`ui`と`main`、`infra/shell.rs`、`infra/shell_thumbnail.rs`にはテストがない。
WinUIのウィンドウ、別のプログラムの起動、Windowsのシェルのサムネイル機能が要るためである。
画面に関わる判断は、テストできるように`explorer`と`domain`に置く。

## 外部とのやり取り

- フォルダの読み込み: `std::fs::read_dir`を使う。
- フォルダの監視: `notify`クレートを使う。Windowsでは`ReadDirectoryChangesW`で、今のフォルダの直下だけを監視する。
- ドライブの一覧: `A:\`から`Z:\`のうち、存在するものを並べる。
- ファイルを開く: `explorer.exe <パス>`を起動する。Windowsが既定のアプリで開く。
- サムネイル: `Image::source_file`に画像ファイルのパスを渡す。読み込みとデコードはWinUIが行う。
  WinUIは表示の大きさでデコードするので、大きな写真が多くてもメモリは増えにくい。
  4000×3000のJPEG 60枚のフォルダで、130MBだった。
- シェルのサムネイル: 画像ファイル以外は、`windows`クレートで`IShellItemImageFactory::GetImage`を呼ぶ。
  エクスプローラーと同じく、アプリが登録したサムネイル機能（`.clip`ならClip Studio Paint）が使われる。
  `SIIGBF_THUMBNAILONLY`を付けるので、サムネイル機能のない種類では何も返らず、アプリのアイコンを出す。
  受け取った`HBITMAP`を`infra/bitmap.rs`の`encode_bmp`でBMPのバイト列にし、`Image::source_data`に渡す。
  1枚は最大256×256pxで、BMPのまま持つので、1枚あたり最大256KBのメモリを使う。
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

### フォルダの変化の反映

1. `ExplorerWindow`は、`update`のたびに今のフォルダを確かめる。変わっていれば、`FolderWatcher::start`で監視を始める。
   前のフォルダの待ちの`ComponentTask`は取り消し、前の`FolderWatcher`は捨てる。
2. `notify`のスレッドは、変化のたびにチャネルへ通知を送る。
3. `spawn_background`のタスクが、スレッドプールで`Changes::wait`を呼ぶ。最初の通知のあと、200ms通知が止まるまで待つ。
   `windows-reactor`の`LocalSender`は別スレッドに渡せないので、待ちのタスクの戻り値でUIのスレッドへ伝える。
4. タスクが`Message::FolderChanged(フォルダ)`を返す。今のフォルダと同じなら、`Explorer::refresh`で読み直し、次の待ちのタスクを始める。
5. `Explorer::refresh`は、選択していた項目をパスで探し直す。ダブルクリックの数え方はリセットする。

### グリッドとサムネイル

1. 切り替えボタンが`Message::ToggleViewMode`を送り、`Explorer::toggle_view_mode`が`ViewMode`を変える。
2. `ViewMode::Grid`のとき、`ui`は`StackPanel`の代わりに`VariableSizedWrapGrid`にタイルを並べる。
3. `Entry::is_image`が真のファイルは、タイルに`Image`でサムネイルを出す。それ以外は大きなアイコンを出す。
4. タイルの選択とダブルクリックは、一覧の行と同じ`Message::RowPressed`で扱う。

### フォルダの表紙

フォルダのタイルには、そのフォルダの直下で名前順に最初の画像を、表紙として出す。

1. 今のフォルダが変わったときと、読み直したときに、`ExplorerWindow::load_covers`が`spawn_background`でタスクを始める。
   前のタスクは取り消す。
2. タスクは、スレッドプールで`folder_covers`を呼ぶ。`Explorer::folders`の各フォルダを読み、最初の画像を探す。
   フォルダごとに読み込みが要るので、UIのスレッドでは行わない。
3. タスクが`Message::CoversFound(フォルダ, 表紙の組)`を返す。`Explorer::set_covers`は、今のフォルダの分だけを受け取る。
4. `ui`は、`Explorer::cover`で表紙を引き、`Image`で出す。表紙の左下に、フォルダのアイコンを重ねる。

### 画像以外のファイルのサムネイル

1. グリッドのときだけ、`ExplorerWindow::load_thumbnails`が`spawn_background`でタスクを始める。
   始めるのは、今のフォルダが変わったとき、読み直したとき、グリッドに切り替えたときである。前のタスクは取り消す。
   一覧ではサムネイルを使わず、1つ作るのにも時間がかかるので、一覧のときは始めない。
2. タスクは、`Explorer::non_image_files`の各ファイルについて、`collect_thumbnails`で`shell_thumbnail`を呼ぶ。
3. タスクが`Message::ThumbnailsMade(フォルダ, サムネイルの組)`を返す。`Explorer::set_thumbnails`は、今のフォルダの分だけを受け取る。
4. `ui`は、`Explorer::thumbnail`でBMPのバイト列を引き、`EncodedImage`にして`Image`で出す。
   同じ`Arc<[u8]>`を毎回渡すので、`windows-reactor`は描き直しのたびにデコードし直さない。

## windows-reactor 0.100.0の制約と、その対応

- `TreeView`は、展開のイベントがなく、クリックではノードの文字しか分からない。そこで、ツリーは字下げしたボタンの行で作る。
- `BreadcrumbBar`は、クリックされたラベルしか通知しない。同じ名前のフォルダがパスに2つあるときは、深いほうの親へ移る。
- 一覧は`ListView`ではなく`StackPanel`で作る。理由は[ADR 001](adr/adr-001-stackpanel-for-file-list.md)にある。
- グリッドも、同じ理由で`GridView`ではなく`VariableSizedWrapGrid`で作る。
- `StackPanel`と`VariableSizedWrapGrid`は、項目を仮想化しない。項目がとても多いフォルダでは遅くなる。
  グリッドでは、見えていない画像も読み込まれる。
- フォルダの監視は直下だけなので、子フォルダの中に画像を足しても、そのフォルダの表紙は変わらない。
  今のフォルダを読み直したときに変わる。
- 一覧は、キーボードの矢印キーでは選べない。

## ADRの一覧

- [ADR 001: StackPanel for the File List](adr/adr-001-stackpanel-for-file-list.md)
