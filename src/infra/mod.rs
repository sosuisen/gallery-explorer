//! Access to the file system and to the favorites file.

mod app_icon;
mod bitmap;
mod file_system;
mod image_uri;
mod shell;
mod shell_thumbnail;
mod store;
mod watcher;

pub use app_icon::show_app_icon;
pub use file_system::{child_folders, collect_thumbnails, drives, folder_covers, read_folder};
pub use image_uri::image_uri;
pub use shell::{move_to_recycle_bin, open_with_default_app, show_in_explorer};
pub use shell_thumbnail::shell_thumbnail;
pub use store::FavoritesStore;
pub use watcher::{Changes, FolderWatcher};
