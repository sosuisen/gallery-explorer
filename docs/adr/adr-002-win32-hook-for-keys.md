# ADR 002: Win32 Hook for Cursor Keys and Tab for Scrolling

## Status

Accepted

## Context

### キーのイベントがない

GalleryExplorerでは、ファイルの一覧で上下キー、グリッドで上下左右のキーを押して、選択を動かす。
Enterを押すと、選んだ項目を開く。
GalleryExplorerは、画面を`windows-reactor` 0.100.0で作っている。
`windows-reactor` 0.100.0のコントロールには、キーのイベント（WinUIの`KeyDown`）がない。
キーを扱う手段は、キーボードアクセラレーター（`KeyAccelerator`）だけである。
しかし、`AcceleratorKey`で選べるのは、`R`、テンキー、`Enter`などに限られ、矢印キーはない。
そのため、`windows-reactor`の公開APIだけでは、矢印キーを受け取れない。

### スクロールさせる手段がない

矢印キーで選んだ項目が画面の外にあれば、見える位置までスクロールする必要がある。
一覧とグリッドは、[ADR 001](adr-001-stackpanel-for-file-list.md)により、`ScrollViewer`の中の`StackPanel`と`VariableSizedWrapGrid`で作っている。
`windows-reactor` 0.100.0の`ScrollViewer`には、スクロールの位置を変える方法（WinUIの`ChangeView`）がない。
要素を見える位置へ動かす方法（WinUIの`StartBringIntoView`）も、公開されていない。
公開されているのは、`ElementRef::request_focus`によるフォーカスの移動だけである。
`request_focus`を使えるのは、`Button`などの一部のコントロールで、`Border`は使えない。

`request_focus`は、WinUIの`Focus(FocusState.Programmatic)`を呼ぶ。
WinUIは、キーボードでフォーカスを受けた要素を、見える位置までスクロールする。
しかし、プログラムでフォーカスした要素はスクロールしない。
実機で確かめると、フォーカスは移った（結果は`Ok(true)`）が、一覧はスクロールしなかった。

## Decision

キーは、Win32の`WH_GETMESSAGE`のフックで受け取る。
スクロールは、透明なボタンへのフォーカスと、自分のウィンドウへ送るTabで起こす。

### キーの受け取り

`ExplorerWindow::create`が、UIのスレッドに`WH_GETMESSAGE`のフックを付ける。
フックは、スレッドのメッセージキューから取り出される`WM_KEYDOWN`を、WinUIより先に見る。
矢印キーとEnterなら、`Message::KeyPressed`として`ExplorerWindow`に送る。
送ったキーは`WM_NULL`に変え、WinUIには渡さない。
フォーカスのあるボタンがEnterで押されたり、矢印キーでフォーカスが動いたりしないためである。

### スクロール

1. `ui`は、選択した項目に、透明で幅1pxの`Button`を2つ重ねる。
2. 選択が動くと、`use_effect`が、新しい画面が出たあとで1つ目のボタンに`request_focus`でフォーカスを移す。
3. フォーカスが移ったら、50ms後に、キーを受けたウィンドウへ`PostMessageW`でTabを送る。
4. WinUIは、Tabでキーボードフォーカスを2つ目のボタンに移し、その項目を見える位置までスクロールする。

50ms待つのは、新しいボタンの配置が決まる前にTabを送ると、前に選んでいた項目へスクロールしたためである。
2つのボタンは同じ項目に置く。`PostMessageW`で送るキーではShiftを押せず、フォーカスは前にしか進められないためである。
Tabはこのアプリのウィンドウに直接送るので、ほかのアプリには届かない。

`windows-reactor`がキーのイベントか、スクロールの方法に対応したら、この決定を見直す。

## Alternatives

### Alternative 1: キーボードアクセラレーターを使う

`windows-reactor`の公開APIだけで済む。
しかし、`AcceleratorKey`に矢印キーがないので、選ばなかった。

### Alternative 2: `WH_KEYBOARD`のフックを使う

キーボードからの入力だけを受け取るフックである。
しかし、`PostMessageW`で送ったキーは見えない。
自分で送ったキーでの動作確認ができないので、選ばなかった。

### Alternative 3: ファイルの一覧を`ListView`に戻す

`ListView`は、矢印キーでの選択とスクロールを自分で行う。
しかし、[ADR 001](adr-001-stackpanel-for-file-list.md)のとおり、フォルダを移るたびに350ms以上のアニメーションが起きる。
`windows-reactor` 0.100.0では止められないので、選ばなかった。

### Alternative 4: プログラムでのフォーカスだけでスクロールさせる

ボタンは1つで足り、Tabも要らない。
しかし、WinUIはプログラムでフォーカスした要素をスクロールしない。実機でもスクロールしなかったので、選ばなかった。

### Alternative 5: すべての項目にボタンを置く

ボタンが前もって配置されるので、50ms待たずにTabを送れる。
しかし、項目ごとにボタンが増え、項目の多いフォルダでは表示が遅くなる。
選択した項目にだけ置けば足りるので、選ばなかった。

### Alternative 6: `windows-reactor`を変更して使う

`windows-reactor`をフォークし、キーのイベントと`ChangeView`を足せば、回避策が要らない。
しかし、プレビュー版のライブラリのフォークを保守する手間が大きいので、選ばなかった。

## Confidence Level

確信度は低めである。
キーの受け取りは、Win32の標準のしくみなので、安定している。
一方、スクロールは、WinUIのフォーカスの振る舞いと、50msという待ち時間に頼っている。
実機では、キーを送るたびに正しくスクロールした。
ただし、キーを押し続けたときや、遅いPCでの振る舞いは確かめていない。

次のどれかが起きたら、この決定を見直す。

- `windows-reactor`が、キーのイベントか、矢印キーのキーボードアクセラレーターに対応した。
  このときは、フックをやめる。
- `windows-reactor`が、`ScrollViewer`のスクロールの位置か、`StartBringIntoView`に対応した。
  このときは、ボタンとTabをやめる。
- 50ms待っても、前の項目へスクロールすることがある。

## Consequences

### 良い結果

- 一覧とグリッドで、矢印キーで選び、Enterで開ける。
- 選んだ項目が画面の外にあっても、見える位置までスクロールする。
- 選択の動きは`domain`の`move_selection`にあり、テストで確かめられる。

### 悪い結果

- 矢印キーとEnterは、どこにフォーカスがあっても、ファイルの一覧とグリッドの操作になる。
  表示順のメニューとお気に入りの一覧は、キーでは操作できない。
- Tabを使うので、WinUIはキーボードで操作中と見なす。
  矢印キーのあとにフォルダを移ると、ツールバーの最初のボタンにフォーカスの枠が出る。
- フック、`unsafe`のWin32の呼び出し、タイマーが`ui.rs`に増える。
  これらはテストできないので、実機で確かめるしかない。

### 中立的な結果

- 選択した項目には、見えないボタンが2つ重なる。幅は1pxなので、項目のクリックはほとんど妨げない。
- ほかのキーの操作を足すときも、同じフックで受け取ることになる。
