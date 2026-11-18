# ADR 001: StackPanel for the File List

## Status

Accepted

## Context

### ListViewのアニメーション

GalleryExplorerの右側の一覧は、今のフォルダの中身を並べる。
最初は、一覧をWinUIの`ListView`で作った。
`ListView`は、既定で`ItemContainerTransitions`にアニメーションを持つ。
項目が変わると、古い項目がフェードアウトし、新しい項目がアニメーションで現れる。
フォルダを移るたびに一覧の項目がすべて入れ替わるので、このアニメーションが毎回起きる。

ダブルクリックの直後から50msごとに画面を撮って確かめた。
`ListView`では、ダブルクリックから350ms後でも、新しいフォルダの項目がまだ出ていなかった。
ユーザーは、画面遷移のアニメーションを不要とした。
不要にできないなら、速くすることを求めた。

### windows-reactorの制約

GalleryExplorerは、画面を`windows-reactor` 0.100.0で作っている。
WinUIでは、`ListView`の`ItemContainerTransitions`を空にすれば、アニメーションが止まる。
しかし、`windows-reactor` 0.100.0の`ListView`には、`ItemContainerTransitions`を設定する方法がない。
アニメーションの長さを変える方法もない。
`windows-reactor`の内部のWinUIのバインディングにも、`Transitions`系のプロパティの設定は含まれていない。

### ItemsRepeaterの制約

`ItemsRepeater`は、既定でアニメーションを持たず、項目を仮想化できる。
しかし、`windows-reactor` 0.100.0は、`ItemsRepeater`の各行を`ContentControl`で包む。
この`ContentControl`は`HorizontalContentAlignment`を設定していないので、行が左に寄る。
行の幅が文字の幅までしかなく、行の右側の余白を押しても反応しなかった。
`windows-reactor`の公開APIからは、この`ContentControl`を変えられない。

## Decision

一覧を、`ScrollViewer`の中の`StackPanel`で作る。
`StackPanel`は既定で`ChildrenTransitions`を持たないので、フォルダを移ったときにアニメーションが起きない。
各行は`Border`で作り、`keyed_children`に項目の名前をキーとして並べる。

`ListView`が持っていた選択とダブルクリックは、自前で扱う。
行の`Border`は、左ボタンの押下のたびに`Message::RowPressed(行, 時刻)`を送る。
`Explorer::press_row`は、押された行を`selected`にし、`DoubleClick`でダブルクリックを判定する。
選択された行は、灰色の背景と、左端の`ThemeBrush::Accent`の線で示す。

`windows-reactor`の`ListView`が、アニメーションを止める方法に対応したら、この決定を見直す。
止める方法とは、`ItemContainerTransitions`を空にする設定、またはそれと同じ効果を持つ設定である。

## Alternatives

### Alternative 1: ListViewのまま使う

選択、キーボード操作、マウスを乗せたときのハイライトを、WinUIの標準のまま使える。
しかし、フォルダを移るたびに350ms以上のアニメーションが起きる。
`windows-reactor` 0.100.0では止められないので、選ばなかった。

### Alternative 2: ItemsRepeaterを使う

アニメーションがなく、項目を仮想化できる。
しかし、`windows-reactor` 0.100.0では行が横に広がらず、行の右側の余白を押せない。
エクスプローラーの一覧として使いにくいので、選ばなかった。

### Alternative 3: ItemsRepeaterの行に、ウィンドウの幅から計算した幅を指定する

`ViewContext::on_window_size`でウィンドウの幅を受け取り、行の`width`に指定すれば、行を横に広げられる。
しかし、左側の幅や余白の値を幅の計算に重ねて書くことになる。
レイアウトを変えるたびに計算も直す必要があり、ずれやすいので、選ばなかった。

### Alternative 4: windows-reactorを変更して使う

`windows-reactor`をフォークし、`ItemContainerTransitions`の設定を足せば、`ListView`のまま使える。
しかし、プレビュー版のライブラリのフォークを保守する手間が大きいので、選ばなかった。

## Confidence Level

確信度は中程度である。
今の`windows-reactor` 0.100.0では、アニメーションをなくし、行の全体を押せる方法として、これが最も簡単である。
一方で、`windows-reactor`はプレビュー版で、APIが大きく変わり続けている。

次のどれかが起きたら、この決定を見直す。

- `windows-reactor`の`ListView`が、アニメーションを止める方法に対応した。
  このときは、`ListView`に戻すことを第一に検討する。
- `windows-reactor`の`ItemsRepeater`が、行を横に広げられるようになった。
  このときは、仮想化のために`ItemsRepeater`を検討する。
- 項目がとても多いフォルダで、一覧の表示が遅いことが問題になった。

## Consequences

### 良い結果

- フォルダを移ると、新しい項目がすぐに出る。
  ダブルクリックの直後に撮った最初の画面で、新しい項目が出ていた。
- 行が横いっぱいに広がり、行のどこを押しても選択とダブルクリックが効く。
- 選択が`Explorer`の状態になったので、選択の振る舞いを`explorer.rs`のテストで確かめられる。

### 悪い結果

- 一覧で、矢印キーによる選択ができない。
- マウスを乗せたときのハイライトがない。
- 選択の見た目を自前で描くので、WinUIの標準の`ListView`とは少し見た目が違う。
- `StackPanel`は項目を仮想化しないので、項目がとても多いフォルダでは表示が遅くなる。
  ただし、`windows-reactor` 0.100.0の`ListView`も、`collection_slot`で渡した項目を仮想化しないので、この点は変更前と同じである。

### 中立的な結果

- キーボード操作やハイライトが必要になったら、自前で足すことになる。
  そのときは、`ListView`に戻せるかを先に確かめる。
