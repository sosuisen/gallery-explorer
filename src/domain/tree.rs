use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One visible row of the folder tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeRow {
    pub path: PathBuf,
    pub depth: usize,
    pub expanded: bool,
}

/// The expanded folders of the folder tree, each with its child folders.
///
/// A folder that is not expanded holds no children. They are read again on the next expand.
#[derive(Debug, Default)]
pub struct Tree {
    expanded: BTreeMap<PathBuf, Vec<PathBuf>>,
}

impl Tree {
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains_key(path)
    }

    pub fn expand(&mut self, path: &Path, children: Vec<PathBuf>) {
        self.expanded.insert(path.to_path_buf(), children);
    }

    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
    }

    /// Returns the visible rows from top to bottom. A folder comes before its children.
    pub fn rows(&self, roots: &[PathBuf]) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        for root in roots {
            self.push_rows(root, 0, &mut rows);
        }
        rows
    }

    fn push_rows(&self, path: &Path, depth: usize, rows: &mut Vec<TreeRow>) {
        let children = self.expanded.get(path);
        rows.push(TreeRow {
            path: path.to_path_buf(),
            depth,
            expanded: children.is_some(),
        });
        for child in children.into_iter().flatten() {
            self.push_rows(child, depth + 1, rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, depth: usize, expanded: bool) -> TreeRow {
        TreeRow {
            path: PathBuf::from(path),
            depth,
            expanded,
        }
    }

    // 何も展開していないツリーには、ルートだけが並ぶ。
    #[test]
    fn tree_with_nothing_expanded_shows_only_the_roots() {
        let tree = Tree::default();

        let rows = tree.rows(&[PathBuf::from(r"C:\"), PathBuf::from(r"D:\")]);

        assert_eq!(rows, [row(r"C:\", 0, false), row(r"D:\", 0, false)]);
    }

    // 展開したフォルダの子は、そのフォルダのすぐ下に1段深く並ぶ。
    #[test]
    fn children_of_an_expanded_folder_follow_it_one_level_deeper() {
        let mut tree = Tree::default();
        tree.expand(Path::new(r"C:\"), vec![PathBuf::from(r"C:\a")]);
        tree.expand(Path::new(r"C:\a"), vec![PathBuf::from(r"C:\a\b")]);

        let rows = tree.rows(&[PathBuf::from(r"C:\"), PathBuf::from(r"D:\")]);

        assert_eq!(
            rows,
            [
                row(r"C:\", 0, true),
                row(r"C:\a", 1, true),
                row(r"C:\a\b", 2, false),
                row(r"D:\", 0, false),
            ]
        );
    }

    // 折りたたんだフォルダの子は、並ばない。
    #[test]
    fn children_of_a_collapsed_folder_are_not_shown() {
        let mut tree = Tree::default();
        tree.expand(Path::new(r"C:\"), vec![PathBuf::from(r"C:\a")]);

        tree.collapse(Path::new(r"C:\"));

        assert!(!tree.is_expanded(Path::new(r"C:\")));
        assert_eq!(tree.rows(&[PathBuf::from(r"C:\")]), [row(r"C:\", 0, false)]);
    }
}
