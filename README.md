# GalleryExplorer

windows-reactor https://github.com/microsoft/windows-rs/tree/master/crates/libs/reactor の実証試験です。

A file explorer for Windows with favorites. It is written in Rust with
[windows-reactor](https://crates.io/crates/windows-reactor), a declarative WinUI 3 library.

`windows-reactor` is a preview. Its API can change.

## Features

- A folder tree with the drives at the top.
- A list of the items in the current folder.
- A breadcrumb bar and an "up" button.
- Favorites: add the current folder, remove it, and jump to it. Drag them up or down to reorder
  them. They are kept between runs.
- A double click opens a folder in the list, or opens a file with its default app.
- A grid view with thumbnails of image files. A button switches between the list and the grid.
  A folder shows its first image. Other files, such as `.clip` or videos, show the thumbnail
  that the Windows shell makes, as File Explorer does.
- Live updates: files that are added to or removed from the open folder show up at once.

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
- `src/infra/shell.rs`: the calls that open a file with its default app, and show an item in File Explorer.
- `src/infra/shell_thumbnail.rs`: the calls that ask the Windows shell for a thumbnail.
  The result depends on the thumbnail handlers installed on the machine.
