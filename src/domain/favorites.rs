use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The user's favorite folders, in the order they were added.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Favorites(Vec<PathBuf>);

impl Favorites {
    pub fn contains(&self, path: &Path) -> bool {
        self.0.iter().any(|favorite| favorite == path)
    }

    /// Adds the folder when it is not a favorite, and removes it when it is.
    pub fn toggle(&mut self, path: &Path) {
        if self.contains(path) {
            self.remove(path);
        } else {
            self.0.push(path.to_path_buf());
        }
    }

    pub fn remove(&mut self, path: &Path) {
        self.0.retain(|favorite| favorite != path);
    }

    /// Puts the favorites in `order`.
    ///
    /// `order` must hold the same folders as the favorites. It comes from the list of
    /// favorites on the screen, after the user dragged one of them.
    pub fn reorder(&mut self, order: Vec<PathBuf>) {
        assert!(
            order.len() == self.0.len() && order.iter().all(|path| self.contains(path)),
            "the new order must hold the same folders"
        );
        self.0 = order;
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // お気に入りでないフォルダを切り替えると、末尾に追加される。
    #[test]
    fn toggling_a_new_folder_adds_it_at_the_end() {
        let mut favorites = Favorites::default();

        favorites.toggle(Path::new(r"C:\a"));
        favorites.toggle(Path::new(r"C:\b"));

        assert_eq!(favorites.paths(), [Path::new(r"C:\a"), Path::new(r"C:\b")]);
    }

    // お気に入りのフォルダを切り替えると、取り除かれる。
    #[test]
    fn toggling_a_favorite_folder_removes_it() {
        let mut favorites = Favorites::default();
        favorites.toggle(Path::new(r"C:\a"));

        favorites.toggle(Path::new(r"C:\a"));

        assert!(!favorites.contains(Path::new(r"C:\a")));
    }

    // 削除すると、そのフォルダだけが取り除かれる。
    #[test]
    fn removing_takes_away_only_that_folder() {
        let mut favorites = Favorites::default();
        favorites.toggle(Path::new(r"C:\a"));
        favorites.toggle(Path::new(r"C:\b"));

        favorites.remove(Path::new(r"C:\a"));

        assert_eq!(favorites.paths(), [Path::new(r"C:\b")]);
    }

    // 並べ替えると、お気に入りは渡した順序になる。
    #[test]
    fn reordering_puts_the_favorites_in_the_given_order() {
        let mut favorites = Favorites::default();
        favorites.toggle(Path::new(r"C:\a"));
        favorites.toggle(Path::new(r"C:\b"));
        favorites.toggle(Path::new(r"C:\c"));

        favorites.reorder(vec![
            PathBuf::from(r"C:\c"),
            PathBuf::from(r"C:\a"),
            PathBuf::from(r"C:\b"),
        ]);

        assert_eq!(
            favorites.paths(),
            [Path::new(r"C:\c"), Path::new(r"C:\a"), Path::new(r"C:\b")]
        );
    }
}
