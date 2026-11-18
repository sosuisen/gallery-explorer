//! Rules of the explorer that need no file system and no UI.

mod breadcrumb;
mod double_click;
mod entry;
mod favorites;
mod selection;
mod tree;

pub use breadcrumb::{crumbs, display_name, resolve_crumb};
pub use double_click::DoubleClick;
pub use entry::{Entry, EntryKind, SortOrder, sort_entries};
pub use favorites::Favorites;
pub use selection::{Arrow, move_selection};
pub use tree::{Tree, TreeRow};
