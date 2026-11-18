//! Access to the file system and to the favorites file.

mod file_system;
mod store;

pub use file_system::{child_folders, drives, open_with_default_app, read_folder};
pub use store::FavoritesStore;
