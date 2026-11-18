//! Access to the file system and to the favorites file.

mod file_system;
mod shell;
mod store;
mod watcher;

pub use file_system::{child_folders, drives, folder_covers, read_folder};
pub use shell::open_with_default_app;
pub use store::FavoritesStore;
pub use watcher::{Changes, FolderWatcher};
