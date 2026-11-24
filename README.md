# GalleryExplorer

windows-reactor https://github.com/microsoft/windows-rs/tree/master/crates/libs/reactor の実証試験です。

A file explorer for Windows with favorites. It is written in Rust with
[windows-reactor](https://crates.io/crates/windows-reactor), a declarative WinUI 3 library.

`windows-reactor` is a preview. Its API can change.

The app uses a copy of `windows-reactor` 0.100.0 in `vendor/windows-reactor`, not the one on
crates.io. The copy adds dragging files to other apps. The changes are listed in
[vendor/windows-reactor/GALLERY_EXPLORER.md](vendor/windows-reactor/GALLERY_EXPLORER.md).

## Features

- A folder tree with the drives at the top.
- A list of the items in the current folder.
- A breadcrumb bar and an "up" button.
- Favorites: add the current folder, remove it, and jump to it. Drag them up or down to reorder
  them. They are kept between runs.
- A double click opens a folder in the list, or opens a file with its default app.
- The cursor keys move the selection. The list uses the up and down keys, and the grid uses all
  four. Enter opens the selected item, as a double click does. Backspace goes up to the
  parent folder.
- A grid view with thumbnails of image files. A button switches between the list and the grid.
  A folder shows its first image. Other files, such as `.clip` or videos, show the thumbnail
  that the Windows shell makes, as File Explorer does. The list shows the same thumbnails at
  60% of the size.
- A sort menu: by name or by modified time, each in ascending or descending order. Folders
  always come before files.
- A right-click menu opens an item in File Explorer. A file is shown selected in its folder.
  The menu of an item in the list or the grid can also move it to the Recycle Bin.
- Drag a file or a folder from the list or the grid to File Explorer or to another app. The
  keys that pick copy, move or a shortcut are the same as in File Explorer.
- Live updates: files that are added to or removed from the open folder show up at once.
- An app icon in File Explorer, on the taskbar, and in the title bar.

## Requirements

- Windows 10 version 1809 or later, or Windows 11.
- The Windows App Runtime 2.x.
- Rust 1.95 or later.

## Run

```
cargo run
```

The favorites are saved in `%APPDATA%\GalleryExplorer\favorites.json`.

## Development

Run these after each change:

```
cargo fmt
cargo clippy --all-targets
cargo test
```

The design is described in [docs/architecture.md](docs/architecture.md).

## Coverage and mutation testing

Install the tools once:

```
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo install cargo-mutants --locked
```

Measure the coverage:

```
cargo cov            # a table per file
cargo cov --html     # a report per line in target/llvm-cov/html/index.html
```

Run the mutation tests. One run must stay at 20 mutants or fewer, so run them by file:

```
cargo mutants -f "src/domain/breadcrumb.rs" -f "src/domain/entry.rs" -f "src/domain/favorites.rs"
cargo mutants -f "src/domain/double_click.rs" -f "src/domain/tree.rs"
cargo mutants -f "src/infra/*.rs"
cargo mutants -f "src/explorer.rs" --shard 0/2
cargo mutants -f "src/explorer.rs" --shard 1/2
```

These files are left out of both. No ordinary test can run them.

- `src/main.rs`: the entry point, which starts the WinUI app.
- `src/ui.rs`: the window, which builds WinUI controls and handles their events.
- `src/infra/shell.rs`: the calls that open a file with its default app, show an item in File Explorer,
  and move an item to the Recycle Bin.
- `src/infra/shell_thumbnail.rs`: the calls that ask the Windows shell for a thumbnail.
  The result depends on the thumbnail handlers installed on the machine.
- `src/infra/app_icon.rs`: the calls that give the window the icon in the exe.
