//! Rules of the explorer that need no file system and no UI.

mod breadcrumb;
mod double_click;
mod entry;
mod favorites;
mod tree;

pub use breadcrumb::{crumbs, display_name, resolve_crumb};
pub use double_click::DoubleClick;
pub use entry::{Entry, EntryKind, sort_entries};
pub use favorites::Favorites;
pub use tree::{Tree, TreeRow};
