# GalleryExplorerでの変更

このフォルダは、crates.ioの`windows-reactor` 0.100.0のコピーである。
元は、windows-rsのコミット`a57cc321bfdbac6bb98c488ba49f399da8fcb98a`の`crates/libs/reactor`である。
GalleryExplorerの`Cargo.toml`は、`[patch.crates-io]`でこのコピーを使う。
理由は[ADR 003](../../docs/adr/adr-003-vendored-windows-reactor-for-drag-out.md)にある。

0.100.0に足したのは、要素からほかのアプリへファイルとフォルダをドラッグする機能である。
`Border::drag_storage_items(DragStorageItems)`で使う。

## 変えたファイル

`src/generated.rs`、`src/native/winui/generated.rs`、`src/native/winui/bindings.rs`は、生成されたファイルである。
生成ツールは配布物にないので、手で直した。
版を上げるときは、新しい版に同じ変更を移す。

### `Cargo.toml`

- `windows` 0.62.2を足した。
  `DataPackage`、`StorageFile`、`StorageFolder`を使うためである。
- `windows-collections` 0.3.2を、`windows-collections-03`という名前で足した。
  `windows` 0.62の`IIterable`を作るためである。

### `src/element.rs`

- `DragStorageItems`を足した。
  ドラッグで渡すパスの一覧である。

### `src/generated.rs`

`Border`の`drop_policy`と同じ形で、`drag_storage_items`を足した。

- `Border`の欄と、ビルダーの`drag_storage_items`
- `into_parts`、`props_match`、`BorderMountedProps`とその`PartialEq`
- `visit_properties`の`PropertyId::BorderCanDrag`
- `PropertyId::BorderCanDrag`
- `PropertyValue::DragStorageItems`と`PropertyValueRef::DragStorageItems`、その比較、`into_owned`、`From<DragStorageItems>`
- `BORDER_PROPERTIES`の記述子

### `src/native/winui/bindings.rs`

- `IUIElement`の`SetCanDrag`、`DragStarting`、`RemoveDragStarting`に、型とラッパーを付けた。
  vtableの欄の順は変えていない。
- `DragStartingEventArgs`と`IDragStartingEventArgs`を足した。
  IIDは`ad17bace-9613-5666-a31b-79a73fba77cf`である。
  メソッドの順は、Windows App SDK 1.7の`Microsoft.UI.Xaml.winmd`で確かめた。
  `Data`は、`DataPackage`のバインディングがないので、`IInspectable`を返す。

### `src/native/winui/generated.rs`

- `set_property`と`clear_property`に、`BorderCanDrag`の腕を足した。
  `BorderAllowDrop`と同じく`UnsupportedKind`を返す。
  実際の処理は`mod.rs`が先に行う。

### `src/native/winui/mod.rs`

- `mod drag_source;`を足した。
- `WinUiRuntime`に`drag_sources`を足した。
- `SetProperty`と`ClearProperty`で、`PropertyId::BorderCanDrag`を処理する。
- 要素を消すときと、すべてを片付けるときに、`drag_sources`からも消す。
- `set_drag_storage_items`を足した。

### `src/native/winui/drag_source.rs`

新しいファイルである。
`CanDrag`を設定し、`DragStarting`で`DataPackage`にファイルとフォルダを入れる。
`StorageFile`と`StorageFolder`は別のスレッドで作り、その間は`GetDeferral`でドラッグを待たせる。
