# ADR 003: Vendored windows-reactor for Dragging Files Out

## Status

Accepted

## Context

アプリのファイルとフォルダを、エクスプローラーやほかのアプリのウィンドウへドラッグできるようにしたい。
WinUI 3では、`UIElement.CanDrag`を`true`にし、`DragStarting`イベントで`DragStartingEventArgs.Data`の`DataPackage`にファイルを入れれば、ほかのアプリがファイルを受け取れる。

しかし、画面を作る`windows-reactor` 0.100.0は、`CanDrag`も`DragStarting`も公開していない。
ライブラリの中のバインディング（`src/native/winui/bindings.rs`）でも、`IUIElement`の`CanDrag`、`SetCanDrag`、`DragStarting`、`RemoveDragStarting`の欄は、型のない`usize`のままである。
`DragStartingEventArgs`と`DataPackage`は、バインディングにない。
`windows-reactor`のコードは、windows-rsのリポジトリにある生成ツール`tool_reactor`が作ったものである。
crates.ioの版には生成ツールも入力も含まれないので、コードを生成し直すことはできない。

## Decision

`windows-reactor` 0.100.0を`vendor/windows-reactor`に取り込み、アプリの`Cargo.toml`の`[patch.crates-io]`で差し替える。
取り込んだコードを手で直し、ドラッグの送り出しを足す。

### 足すAPI

`Border`に`drag_storage_items(DragStorageItems)`を足す。
`DragStorageItems`は、ドラッグで渡すファイルとフォルダのパスの一覧である。
既存の`drop_policy`と同じ形のプロパティ（`PropertyId::BorderCanDrag`）として、`src/generated.rs`に足す。

### ドラッグの流れ

1. `drag_storage_items`が設定されると、`src/native/winui/mod.rs`が`drag_source::DragSource::start`を呼ぶ。
   `DragSource`は、要素の`CanDrag`を`true`にし、`DragStarting`を購読する。
2. ユーザーが要素をドラッグすると、WinUIが`DragStarting`を呼ぶ。
   `drag_source.rs`の`fill_drag`は、許す操作をコピー・移動・リンクにし、`GetDeferral`でドラッグを待たせる。
3. 別のスレッドが、パスから`StorageFile`または`StorageFolder`を作り、`DataPackage.SetStorageItemsReadOnly`で入れる。
   `StorageFile`と`StorageFolder`は非同期にしか作れず、UIスレッドで待つとドラッグを止めるためである。
   `DataPackage`はagileなので、別のスレッドから入れられる。
4. そのスレッドは、`DispatcherQueue`でUIスレッドにdeferralの`Complete`を頼む。
   deferralは、WinUIがUIスレッドで作ったものだからである。

`DataPackage`、`StorageFile`、`StorageFolder`には、`windows`クレート0.62の型を使う。
`windows-reactor`の`windows-core`は0.100で、版が違う。
そこで、`DragStartingEventArgs.Data`を`IInspectable`として受け取り、生ポインターで`windows`クレートの`DataPackage`に移す。
2つの版の型は、同じCOMのオブジェクトを指す。

### 差分の記録

取り込んだ版から変えた点は、`vendor/windows-reactor/GALLERY_EXPLORER.md`に書く。
版を上げるときは、この文書に沿って差分を移し直す。

### アプリの側

一覧の行とグリッドのタイルの`Border`に、その項目のパスを`drag_storage_items`で設定する。
すべての項目を同じ`Grid`で包み、スクロール用の透明なボタンは、選択された項目にだけ入れる。
選択された項目だけを`Grid`で包むと、押した項目が選択に変わるときに、WinUIがその項目のコントロールを作り直すためである。
押したコントロールが消えると、ドラッグは始まらない。

## Alternatives

### Alternative 1: Win32のOLEでドラッグを始める

WinUIを通さずに、Win32の`SHDoDragDrop`でドラッグを始める。
`windows-reactor`を直さずに済む。
実際に試したが、ドラッグのループがマウスの移動も離上も受け取れず、ドラッグが終わらなかった。
WinUIのプロセスでは`IsMouseInPointerEnabled`が`TRUE`だった。
マウスの入力がポインターのメッセージになり、OLEのループに届かないためと見ているが、確かめていない。
別のスレッドで動かしても、別のプロセスで動かしても、同じように止まった。

### Alternative 2: upstreamでの対応を待つ

windows-rsにissueを出し、`windows-reactor`がドラッグの送り出しに対応するのを待つ。
手元のコードは増えない。
いつ使えるようになるかが分からないので、選ばなかった。
足した差分をupstreamへのPRにすることは、この決定と両立する。

## Confidence Level

確信度は高い。
WinUIの標準の仕組みを使っており、エクスプローラーへのドラッグを手で確かめた。
`windows-reactor`がドラッグの送り出しに対応したら、`vendor/`をやめて、crates.ioの版に戻す。
`windows-reactor`の新しい版の機能が必要になり、差分を移し直す手間が大きいときも、この決定を見直す。

## Consequences

### 良い結果

- エクスプローラーと同じキーで、コピー・移動・ショートカットの作成ができる。
  同じドライブへは移動、別のドライブへはコピーになる。
- ファイルだけでなく、フォルダもドラッグできる。
- `windows-reactor`のほかの制約も、同じ`vendor/`で直せるようになった。

### 悪い結果

- `windows-reactor`の版を上げるたびに、差分を移し直す必要がある。
  生成されたファイル（`src/generated.rs`、`src/native/winui/generated.rs`、`src/native/winui/bindings.rs`）を手で直している。
- `windows-core`、`windows-collections`の2つの版がビルドに混ざる。
- 取り込んだコードの分だけ、リポジトリが大きくなる。

### 中立的な結果

- 一覧とグリッドのすべての項目が、`Grid`で包まれる。
