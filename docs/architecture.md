# architecture

## アプリの全体像

GalleryExplorerは、お気に入り機能の付いたWindows用のファイルエクスプローラーである。
Rustで書き、画面は`windows-reactor`(WinUI 3の宣言的UIライブラリ、プレビュー版)で作る。

画面は1つのウィンドウだけである。

| 場所 | 内容 |
|---|---|
| 左 | お気に入りの一覧、フォルダのツリー。ウィンドウの上端から、ステータスバーの上まで伸びる |
| 右 | 上から、「上へ」ボタンとパンくずの行、一覧とグリッドの切り替えボタンと表示順のメニューのツールバー、今のフォルダの中身。中身は、一覧か、サムネイルのグリッドで示す |
| 下 | 項目の数、またはエラーの文 |

## レイヤーとモジュール

依存の向きは、`ui` → `explorer` → `infra` → `domain`の一方向である。`domain`は他のどれにも依存しない。

| モジュール | 役割 | 主な型と関数 |
|---|---|---|
| `domain` | ファイルシステムも画面も使わないルール | `Entry`、`SortOrder`、`sort_entries`、`crumbs`、`resolve_crumb`、`display_name`、`Favorites`、`Tree`、`DoubleClick`、`Arrow`、`move_selection` |
| `infra` | ファイルシステムとお気に入りのファイルへのアクセス | `read_folder`、`child_folders`、`folder_covers`、`collect_thumbnails`、`shell_thumbnail`、`image_uri`、`drives`、`FavoritesStore`、`open_with_default_app`、`show_in_explorer`、`FolderWatcher`、`Changes`、`show_app_icon` |
| `explorer` | ウィンドウの状態と、ユーザーの操作 | `Explorer`、`ViewMode` |
| `ui` | 状態をWinUIのコントロールに変える | `ExplorerWindow`、`Message` |
| `main` | エントリーポイント | `main` |

`windows-reactor`は、crates.ioの版ではなく、`vendor/windows-reactor`のコピーを使う。
アプリの`Cargo.toml`の`[patch.crates-io]`で差し替える。
コピーには、ほかのアプリへファイルをドラッグする機能を足してある。
理由は[ADR 003](adr/adr-003-vendored-windows-reactor-for-drag-out.md)に、変えた点は`vendor/windows-reactor/GALLERY_EXPLORER.md`にある。

`ui`と`main`、`infra/shell.rs`、`infra/shell_thumbnail.rs`、`infra/app_icon.rs`にはテストがない。
WinUIのウィンドウ、別のプログラムの起動、Windowsのシェルのサムネイル機能、exeに埋め込んだアイコンが要るためである。
画面に関わる判断は、テストできるように`explorer`と`domain`に置く。

## 外部とのやり取り

- フォルダの読み込み: `std::fs::read_dir`を使う。
- フォルダの監視: `notify`クレートを使う。Windowsでは`ReadDirectoryChangesW`で、今のフォルダの直下だけを監視する。
- ドライブの一覧: `A:\`から`Z:\`のうち、存在するものを並べる。
- ファイルを開く: `explorer.exe <パス>`を起動する。Windowsが既定のアプリで開く。
- エクスプローラーで開く: フォルダは、`explorer.exe <パス>`で開く。
  ファイルは、`ILCreateFromPathW`で項目にし、`SHOpenFolderAndSelectItems`で、選んだ状態で開く。
  `explorer.exe /select,<パス>`は使わない。Windows 11では、フォルダは開くが、ファイルが選ばれなかったためである。
- 右クリックメニュー: Win32の`TrackPopupMenu`で出す。
- 削除: `IFileOperation`の`DeleteItem`で、ごみ箱へ移す。
  フラグは`FOF_ALLOWUNDO`、`FOF_NOCONFIRMATION`、`FOF_WANTNUKEWARNING`、`FOFX_RECYCLEONDELETE`である。
  確認のダイアログは出さない。
  ごみ箱に入らない項目だけは、Windowsが完全に削除してよいかを尋ねる。`FOF_NOCONFIRMATION`だけでは、黙って完全に削除されるためである。
- サムネイル: `image_uri`で画像ファイルのパスを`file:///`のURIにし、`Image::source`に渡す。読み込みとデコードはWinUIが行う。
  `image_uri`は、日本語などASCII以外の文字をそのまま残し、空白・`#`・`%`などの記号だけをエンコードする。
  `Image::source_file`は使わない。ASCII以外の文字をUTF-8でパーセントエンコードするので、WinUIがファイルを開けないためである。
  WinUIは表示の大きさでデコードするので、大きな写真が多くてもメモリは増えにくい。
  4000×3000のJPEG 60枚のフォルダで、130MBだった。
- シェルのサムネイル: 画像ファイル以外は、`windows`クレートで`IShellItemImageFactory::GetImage`を呼ぶ。
  エクスプローラーと同じく、アプリが登録したサムネイル機能（`.clip`ならClip Studio Paint）が使われる。
  `SIIGBF_THUMBNAILONLY`を付けるので、サムネイル機能のない種類では何も返らず、アプリのアイコンを出す。
  受け取った`HBITMAP`を`infra/bitmap.rs`の`encode_bmp`でBMPのバイト列にし、`Image::source_data`に渡す。
  1枚は最大256×256pxで、BMPのまま持つので、1枚あたり最大256KBのメモリを使う。
- お気に入りの保存: `%APPDATA%\GalleryExplorer\favorites.json`に、パスの文字列のJSON配列で保存する。
- アプリのアイコン: `build.rs`が、`winresource`クレートで`assets/icon.ico`をexeにリソースID 1で埋め込む。
  エクスプローラーやデスクトップのショートカットは、このアイコンを示す。
  ウィンドウには、起動後に`infra/app_icon.rs`の`show_app_icon`が`WM_SETICON`で同じアイコンを付ける。タイトルバーとタスクバーは、このアイコンを示す。
  `windows-reactor`の`WindowVisuals::icon`は、0.100.0では値を受け取るだけで使われない。ウィンドウのハンドルも公開されていない。
  そのため、UIスレッドのタイトルバーのあるウィンドウを`EnumThreadWindows`で探す。
  `assets/icon.png`は、`icon.ico`を作り直すときの元の画像で、コードからは使わない。
- 実行時には、Windows App Runtimeがインストールされている必要がある。

## 主な処理の流れ

### 起動

1. `main`が`App::run_component::<ExplorerWindow>`を呼ぶ。
2. `ExplorerWindow::create`が、`drives`でツリーのルートを得る。開始フォルダは`%USERPROFILE%`である。
3. `Explorer::open`が、`FavoritesStore::load`でお気に入りを読み、`read_folder`で開始フォルダを読む。
4. `ExplorerWindow::view`が、`Explorer`の状態から画面を作る。
5. 最初の`view`のあとに1度だけ、効果（`use_effect`）が`show_app_icon`を呼び、ウィンドウにアイコンを付ける。

### 操作

1. コントロールのイベントが`Message`になり、`ExplorerWindow::update`に届く。
2. `update`が、`Explorer`のメソッドを1つ呼ぶ。
3. `windows-reactor`が`view`を呼び直し、変わったコントロールだけを更新する。

### 一覧の行の選択と、ダブルクリックで開く

一覧は、`ScrollViewer`の中の`StackPanel`に、行ごとの`Border`を並べて作る。選択とダブルクリックは自前で扱う。

1. 一覧の行の`Border`が、左ボタンの押下のたびに`Message::RowPressed(行, 時刻)`を送る。
2. `Explorer::press_row`が、押された行を選択にする。選択された行は、灰色の背景と左のアクセント色の線で示す。
3. `Explorer::press_row`が`DoubleClick`に渡す。同じ行への500ms以内の2回目の押下が、ダブルクリックになる。
4. `Explorer::open_selected`で開く。フォルダなら`navigate`する。ファイルならパスを返し、`ui`が`open_with_default_app`で開く。

### キーでの選択と、Enterで開く

`windows-reactor`には、キーのイベントがない。キーボードアクセラレーターはあるが、矢印キーを扱えない。
そこで、`ui`はWin32のフックでキーを受け取る。

1. `ExplorerWindow::create`が、`hook_keys`でUIのスレッドに`WH_GETMESSAGE`のフックを付ける。
   フックは、WinUIより先に、スレッドのメッセージキューから取り出されるキーを見る。
2. 矢印キー、Enter、Backspaceの`WM_KEYDOWN`を、`Message::KeyPressed(Key)`にして送る。
   送ったキーは`WM_NULL`に変え、WinUIには渡さない。
   フォーカスのあるボタンがEnterで押されたり、矢印キーでフォーカスが動いたりしないためである。
3. 矢印キーなら、`Explorer::move_selection`が`domain`の`move_selection`で次の選択を決める。
   グリッドでは、1行のタイルの数を渡す。`ui`は、`on_window_size`で受け取ったウィンドウの幅から、`grid_columns`で数を求める。
   一覧では、左右キーを無視し、1行を1項目として扱う。
4. Enterなら、`Explorer::open_selected`で開く。ダブルクリックと同じ処理である。
5. Backspaceなら、`Explorer::go_up`で親フォルダへ移る。「上へ」ボタンと同じ処理である。

選択した項目が見えないときは、見える位置までスクロールする。
`windows-reactor`は`ScrollViewer`をスクロールできないので、WinUIのキーボードフォーカスを使う。
WinUIは、キーボードでフォーカスを受けた要素を見える位置までスクロールする。プログラムでフォーカスした要素はスクロールしない。

1. `ui`は、すべての項目を`Grid`で包み、選択した項目にだけ、透明で幅1pxのボタンを2つ重ねる（`with_scroll_buttons`）。
   選択していない項目も`Grid`で包むのは、選択が変わっても項目の`Border`を作り直さないためである。
   作り直すと、押した`Border`が消え、その項目のドラッグが始まらない。
2. 矢印キーのたびに`key_moves`を増やす。`use_effect`は、新しい画面が出たあとで1つ目のボタンにフォーカスを移す。
3. フォーカスが移ったら、`press_tab_soon`が50ms後に、キーを受けたウィンドウへTabを送る（`PostMessageW`）。
   新しいボタンの配置が決まる前にTabを送ると、前に選んでいた項目へスクロールしたためである。
4. Tabで、キーボードフォーカスが2つ目のボタンに移り、WinUIがその項目をスクロールして見せる。

2つのボタンは同じ項目に置く。送るキーではShiftを押せないので、フォーカスは前にしか進められないためである。
Tabは、`PostMessageW`でこのアプリのウィンドウに送るので、ほかのアプリには届かない。

### ほかのアプリへのドラッグ

1. `ui`は、一覧の行とグリッドのタイルの`Border`に、その項目のパスを`drag_storage_items`で設定する。
2. `windows-reactor`のコピーは、その`Border`の`CanDrag`を`true`にし、`DragStarting`を購読する。
3. ドラッグが始まると、`DragStarting`が、許す操作をコピー・移動・リンクにし、`GetDeferral`でドラッグを待たせる。
4. 別のスレッドが、パスから`StorageFile`か`StorageFolder`を作り、`DataPackage.SetStorageItemsReadOnly`で入れる。
5. そのスレッドが、`DispatcherQueue`でUIスレッドにdeferralの`Complete`を頼む。ドラッグが進む。

コピー・移動・リンクのどれにするかは、ドロップ先のアプリが、押されたキーで決める。
エクスプローラーへドロップして移動になると、元のファイルは今のフォルダから消える。フォルダの監視が、一覧から消す。
処理の詳細は`vendor/windows-reactor/GALLERY_EXPLORER.md`にある。

### 右クリックメニュー

1. 一覧の行とグリッドのタイルの`Border`は、右ボタンの押下で`Message::ShowItemMenu(パス, フォルダか)`を送る。
   お気に入りの行とツリーの行は、`menu_handler`で`Message::ShowMenu(パス)`を送る。
   一覧とグリッドの項目のない場所は、`background_menu_handler`で、今のフォルダの`Message::ShowMenu`を送る。
2. `update`が`show_context_menu`を呼び、`show_context_menu`が`popup_menu`を呼ぶ。`popup_menu`は、マウスカーソルの位置にWin32のメニューを出し、選ばれた項目の番号を返す。
   メニューが閉じるまで、`TrackPopupMenu`の中でメッセージループが回る。
3. 「エクスプローラーで開く」が選ばれると、`show_in_explorer`がエクスプローラーで開く。
4. 2番目の項目は、パスがお気に入りなら「お気に入りから外す」、そうでなければ「お気に入りに追加」である。
   選ばれると、`Explorer::toggle_favorite`が切り替えて保存する。
5. 一覧とグリッドのメニューには「削除」もある。選ばれると、`move_to_recycle_bin`がごみ箱へ移す。
   一覧からは、フォルダの監視が消す。
   お気に入りとツリーのメニューには「削除」を出さない。

ツリーの行は`Button`なので、行全体を透明な背景の`Border`で包み、そこで右ボタンの押下を受け取る。
`Button`には、押下のイベントがないためである。
お気に入りの行の名前の`Border`には、透明な背景を付ける。
背景のない`Border`は、押下を受け取らないためである。

一覧とグリッドの項目のない場所の右クリックは、`ScrollViewer`を包む透明な背景の`Border`で受け取る。
項目の上の右クリックも、WinUIが親へ伝えるので、項目の`Border`のあとに、この`Border`に届く。
そこで、項目の`press_handler`は`ITEM_MENU_ASKED`を立てる。
`background_menu_handler`は、`ITEM_MENU_ASKED`が立っていればそれを下ろし、メニューを出さない。
`windows-reactor`は、届いた順にイベントを呼ぶので、項目のハンドラーが先に呼ばれる。

### 表示順

1. 表示順のメニューは`ComboBox`である。選ぶと`Message::SortChosen(行)`が届く。
2. `Explorer::set_sort_order`が`SortOrder`を覚え、今の項目を`sort_entries`で並べ直す。選択していた項目は、パスで探し直す。
3. `Explorer`は、フォルダを読むたびに、覚えた`SortOrder`で並べる。そのため、フォルダを移っても、読み直しても、表示順は変わらない。

`read_folder`自身は、いつも名前の昇順で返す。
表紙の1枚目の画像とツリーの子フォルダは、表示順に関係なく名前順で決めるためである。
表示順はファイルに保存しない。アプリを起動するたびに、名前の昇順に戻る。

### お気に入りの並べ替え

お気に入りは、`ListView`に行ごとの`ListViewItem`を並べて作る。
ファイルの一覧と違って`ListView`を使うのは、WinUIのドラッグでの並べ替え（`CanReorderItems`）を使うためである。
項目が変わるときのアニメーションは消せないが、お気に入りはめったに変わらないので問題にならない。

1. 各`ListViewItem`の`tag`に、ファイルかフォルダのパスの文字列を入れる。
2. ドラッグが終わると、`ListView`が`on_reordered`で、新しい順序の`tag`を返す。`ui`はこれを`Message::FavoritesReordered`にする。
3. `Explorer::reorder_favorites`が、`Favorites::reorder`で並べ直し、`FavoritesStore::save`で保存する。

行の名前の部分は、`Button`にしない。`Button`が押下を受け取ると、ドラッグが始まらないためである。
その代わりに、行を選ぶと`Message::FavoriteChosen(行)`が届き、`Explorer::open_favorite`を呼ぶ。
お気に入りがファイルなら、`open_favorite`はそのパスを返し、`ui`が`open_with_default_app`で開く。今のフォルダ、表示、表示順は変えない。
お気に入りがフォルダなら、`open_favorite`はそのフォルダへ移り、表示をグリッドにし、表示順を更新日の降順にする。
表示順はフォルダへ移る前に変えるので、お気に入りのフォルダは最初から更新日の降順で読まれる。
そのあと`ui`は、`load_previews`で表紙とサムネイルを読み込み直す。
お気に入りが今のフォルダと同じだと、フォルダの変化がないので、表紙もサムネイルも自動では読み込まれないためである。
`view`は毎回`selected_index(None)`で選択を消すので、同じ行をもう一度選べる。
削除のボタンは`Button`のままである。

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
5. 一覧の行も、同じ`preview`でサムネイルを出す。大きさは、グリッドの6割（`LIST_PREVIEW`）である。

### フォルダの表紙

フォルダのタイルと一覧の行には、そのフォルダの直下で名前順に最初の画像を、表紙として出す。

1. 今のフォルダが変わったときと、読み直したときに、`ExplorerWindow::load_covers`が`spawn_background`でタスクを始める。
   前のタスクは取り消す。
2. タスクは、スレッドプールで`folder_covers`を呼ぶ。`Explorer::folders`の各フォルダを読み、最初の画像を探す。
   フォルダごとに読み込みが要るので、UIのスレッドでは行わない。
3. タスクが`Message::CoversFound(フォルダ, 表紙の組)`を返す。`Explorer::set_covers`は、今のフォルダの分だけを受け取る。
4. `ui`は、`Explorer::cover`で表紙を引き、`Image`で出す。表紙の左下に、フォルダのアイコンを重ねる。

### 画像以外のファイルのサムネイル

1. `ExplorerWindow::load_thumbnails`が`spawn_background`でタスクを始める。
   始めるのは、今のフォルダが変わったときと、読み直したときである。前のタスクは取り消す。
   一覧もグリッドもサムネイルを出すので、表示の方法は問わない。
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
- キーのイベントがないので、矢印キー、Enter、BackspaceはWin32のフックで受け取る。理由は[ADR 002](adr/adr-002-win32-hook-for-keys.md)にある。
  このキーは、どのコントロールにフォーカスがあっても、ファイルの一覧とグリッドのものになる。
  そのため、表示順のメニューやお気に入りの一覧では、矢印キーとEnterが使えない。マウスで選ぶ。
  文字の入力欄を足すときは注意する。入力欄でも、Backspaceで文字を消せず、親フォルダへ移ってしまう。
- 矢印キーで選んだあとは、WinUIがキーボードで操作していると見なす。
  フォルダを移ると、ウィンドウの最初のコントロールにフォーカスの枠が出る。
- `MenuFlyout`がないので、右クリックメニューはWin32の`TrackPopupMenu`で出す。見た目は、WinUIではなく、古いWin32のメニューになる。
  このメニューは、キーボードでは選べない。キーがメニューに届かないためである。外をクリックすると閉じる。
- 要素の`CanDrag`と`DragStarting`がないので、`vendor/windows-reactor`のコピーに足した。理由は[ADR 003](adr/adr-003-vendored-windows-reactor-for-drag-out.md)にある。
  Win32のOLE（`SHDoDragDrop`）では、WinUIのウィンドウからドラッグを始めても、ドラッグが終わらない。

## ADRの一覧

- [ADR 001: StackPanel for the File List](adr/adr-001-stackpanel-for-file-list.md)
- [ADR 002: Win32 Hook for Cursor Keys and Tab for Scrolling](adr/adr-002-win32-hook-for-keys.md)
- [ADR 003: Vendored windows-reactor for Dragging Files Out](adr/adr-003-vendored-windows-reactor-for-drag-out.md)
