//! The state of the explorer window and the operations the user can do on it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::domain::{DoubleClick, Entry, Favorites, Tree, TreeRow, resolve_crumb};
use crate::infra::{FavoritesStore, child_folders, read_folder};

/// How the items of the current folder are shown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    /// One row per item, with a small icon.
    List,
    /// Tiles in rows, with a thumbnail for each image.
    Grid,
}

pub struct Explorer {
    roots: Vec<PathBuf>,
    current: PathBuf,
    entries: Vec<Entry>,
    /// The first image of each folder in `entries`, by folder path.
    covers: HashMap<PathBuf, PathBuf>,
    selected: Option<usize>,
    view_mode: ViewMode,
    error: Option<String>,
    favorites: Favorites,
    store: FavoritesStore,
    tree: Tree,
    clicks: DoubleClick,
}

impl Explorer {
    /// Starts in `start`. `roots` are the top folders of the tree.
    pub fn open(roots: Vec<PathBuf>, start: &Path, store: FavoritesStore) -> Self {
        let (favorites, error) = match store.load() {
            Ok(favorites) => (favorites, None),
            Err(error) => (
                Favorites::default(),
                Some(format!("お気に入りを読み込めません: {error}")),
            ),
        };
        let mut explorer = Self {
            roots,
            current: start.to_path_buf(),
            entries: Vec::new(),
            covers: HashMap::new(),
            selected: None,
            view_mode: ViewMode::List,
            error: None,
            favorites,
            store,
            tree: Tree::default(),
            clicks: DoubleClick::default(),
        };
        explorer.navigate(start);
        // The favorites error is the older one, but it would be lost behind a clean start.
        explorer.error = explorer.error.or(error);
        explorer
    }

    pub fn current(&self) -> &Path {
        &self.current
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Switches between the list and the grid.
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::List => ViewMode::Grid,
            ViewMode::Grid => ViewMode::List,
        };
    }

    /// The folders in the current folder. Their covers are looked up from these.
    pub fn folders(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|entry| entry.is_folder())
            .map(|entry| entry.path.clone())
            .collect()
    }

    /// Returns the image shown for a folder: its first image, when one was found.
    pub fn cover(&self, entry: &Entry) -> Option<&Path> {
        self.covers.get(&entry.path).map(PathBuf::as_path)
    }

    /// Sets the covers found for the folders in `folder`.
    /// Covers of a folder that is no longer shown arrive late and are ignored.
    pub fn set_covers(&mut self, folder: &Path, covers: Vec<(PathBuf, PathBuf)>) {
        if folder == self.current {
            self.covers = covers.into_iter().collect();
        }
    }

    pub fn favorites(&self) -> &[PathBuf] {
        self.favorites.paths()
    }

    pub fn is_current_favorite(&self) -> bool {
        self.favorites.contains(&self.current)
    }

    pub fn tree_rows(&self) -> Vec<TreeRow> {
        self.tree.rows(&self.roots)
    }

    /// Shows the items of `path`. On failure, the current folder stays and the error is kept.
    pub fn navigate(&mut self, path: &Path) {
        match read_folder(path) {
            Ok(entries) => {
                self.current = path.to_path_buf();
                self.entries = entries;
                self.covers.clear();
                self.selected = None;
                self.clicks.reset();
                self.error = None;
            }
            Err(error) => self.fail(path, &error),
        }
    }

    /// Reads the current folder again. The selected item stays selected while it is still there.
    ///
    /// When the folder cannot be read any more, the list becomes empty and the error is kept.
    pub fn refresh(&mut self) {
        let selected = self
            .selected
            .and_then(|row| self.entries.get(row))
            .map(|entry| entry.path.clone());
        // The rows may move, so a press before the refresh must not pair with one after it.
        self.clicks.reset();
        match read_folder(&self.current) {
            Ok(entries) => {
                self.selected =
                    selected.and_then(|path| entries.iter().position(|entry| entry.path == path));
                self.entries = entries;
            }
            Err(error) => {
                self.entries.clear();
                self.selected = None;
                let current = self.current.clone();
                self.fail(&current, &error);
            }
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current.parent().map(Path::to_path_buf) {
            self.navigate(&parent);
        }
    }

    pub fn crumb_clicked(&mut self, label: &str) {
        if let Some(path) = resolve_crumb(&self.current, label) {
            self.navigate(&path);
        }
    }

    /// Records a press on a list row. The pressed row becomes the selected row.
    ///
    /// A double click on a folder shows that folder. A double click on a file returns its path,
    /// and the caller opens it.
    pub fn press_row(&mut self, row: usize, at: Instant) -> Option<PathBuf> {
        self.selected = Some(row);
        if !self.clicks.press(row, at) {
            return None;
        }
        let entry = self.entries.get(row)?.clone();
        if entry.is_folder() {
            self.navigate(&entry.path);
            None
        } else {
            Some(entry.path)
        }
    }

    /// Expands a collapsed tree folder, and collapses an expanded one.
    pub fn toggle_tree(&mut self, path: &Path) {
        if self.tree.is_expanded(path) {
            self.tree.collapse(path);
        } else {
            self.expand_tree(path);
        }
    }

    /// Shows a tree folder in the list and expands it in the tree.
    pub fn open_tree_folder(&mut self, path: &Path) {
        self.navigate(path);
        if !self.tree.is_expanded(path) {
            self.expand_tree(path);
        }
    }

    /// Adds the current folder to the favorites, or removes it when it is already there.
    pub fn toggle_favorite(&mut self) {
        self.favorites.toggle(&self.current);
        self.save_favorites();
    }

    pub fn remove_favorite(&mut self, path: &Path) {
        self.favorites.remove(path);
        self.save_favorites();
    }

    pub fn report_error(&mut self, message: String) {
        self.error = Some(message);
    }

    fn expand_tree(&mut self, path: &Path) {
        match child_folders(path) {
            Ok(children) => self.tree.expand(path, children),
            Err(error) => self.fail(path, &error),
        }
    }

    fn save_favorites(&mut self) {
        if let Err(error) = self.store.save(&self.favorites) {
            self.error = Some(format!("お気に入りを保存できません: {error}"));
        }
    }

    fn fail(&mut self, path: &Path, error: &std::io::Error) {
        self.error = Some(format!("{}を開けません: {error}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// A folder with one sub folder `photos` (holding `inner`) and one file `note.txt`.
    fn sample() -> TempDir {
        let folder = tempfile::tempdir().unwrap();
        fs::create_dir_all(folder.path().join("photos").join("inner")).unwrap();
        fs::write(folder.path().join("note.txt"), "").unwrap();
        folder
    }

    fn open(folder: &TempDir) -> Explorer {
        let store = FavoritesStore::new(folder.path().join("favorites").join("favorites.json"));
        Explorer::open(vec![folder.path().to_path_buf()], folder.path(), store)
    }

    fn names(explorer: &Explorer) -> Vec<&str> {
        explorer
            .entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect()
    }

    fn double_click(explorer: &mut Explorer, row: usize) -> Option<PathBuf> {
        let at = Instant::now();
        assert_eq!(explorer.press_row(row, at), None);
        explorer.press_row(row, at)
    }

    // フォルダの一覧には、ファイルが含まれない。
    #[test]
    fn folders_do_not_include_files() {
        let folder = sample();

        assert_eq!(open(&folder).folders(), [folder.path().join("photos")]);
    }

    // 見つかった1枚目の画像が、そのフォルダの表紙になる。
    #[test]
    fn found_image_becomes_the_cover_of_its_folder() {
        let folder = sample();
        let mut explorer = open(&folder);
        let photos = folder.path().join("photos");
        let image = photos.join("a.jpg");

        explorer.set_covers(folder.path(), vec![(photos, image.clone())]);

        assert_eq!(
            explorer.cover(&explorer.entries()[0]),
            Some(image.as_path())
        );
        assert_eq!(explorer.cover(&explorer.entries()[1]), None);
    }

    // 前のフォルダのために探した表紙は、無視される。
    #[test]
    fn covers_for_another_folder_are_ignored() {
        let folder = sample();
        let mut explorer = open(&folder);
        let photos = folder.path().join("photos");

        explorer.set_covers(&photos, vec![(photos.clone(), photos.join("a.jpg"))]);

        assert_eq!(explorer.cover(&explorer.entries()[0]), None);
    }

    // フォルダを移ると、前のフォルダの表紙は消える。
    #[test]
    fn covers_are_cleared_when_the_folder_changes() {
        let folder = sample();
        let mut explorer = open(&folder);
        let inner = folder.path().join("photos").join("inner");
        explorer.set_covers(folder.path(), vec![(inner.clone(), inner.join("a.jpg"))]);

        explorer.navigate(&folder.path().join("photos"));

        assert_eq!(explorer.cover(&explorer.entries()[0]), None);
    }

    // 読み直しても、新しい表紙が届くまでは、前の表紙が残る。
    #[test]
    fn covers_stay_through_a_refresh() {
        let folder = sample();
        let mut explorer = open(&folder);
        let photos = folder.path().join("photos");
        let image = photos.join("a.jpg");
        explorer.set_covers(folder.path(), vec![(photos, image.clone())]);

        explorer.refresh();

        assert_eq!(
            explorer.cover(&explorer.entries()[0]),
            Some(image.as_path())
        );
    }

    // 読み直すと、追加されたファイルが一覧に並ぶ。
    #[test]
    fn refreshing_shows_an_added_file() {
        let folder = sample();
        let mut explorer = open(&folder);
        fs::write(folder.path().join("added.jpg"), "").unwrap();

        explorer.refresh();

        assert_eq!(names(&explorer), ["photos", "added.jpg", "note.txt"]);
    }

    // 読み直すと、削除されたファイルは一覧から消える。
    #[test]
    fn refreshing_drops_a_removed_file() {
        let folder = sample();
        let mut explorer = open(&folder);
        fs::remove_file(folder.path().join("note.txt")).unwrap();

        explorer.refresh();

        assert_eq!(names(&explorer), ["photos"]);
    }

    // 前に項目が増えても、選択していた項目は選択されたままである。
    #[test]
    fn selected_item_stays_selected_when_an_item_is_added_before_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(1, Instant::now());
        fs::write(folder.path().join("a.jpg"), "").unwrap();

        explorer.refresh();

        let selected = explorer.selected().unwrap();
        assert_eq!(explorer.entries()[selected].name, "note.txt");
    }

    // 選択していた項目が消えると、選択はなくなる。
    #[test]
    fn selection_is_cleared_when_the_selected_item_is_removed() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(1, Instant::now());
        fs::remove_file(folder.path().join("note.txt")).unwrap();

        explorer.refresh();

        assert_eq!(explorer.selected(), None);
    }

    // 読み直しの前後の押下は、ダブルクリックにならない。
    #[test]
    fn presses_before_and_after_a_refresh_are_not_a_double_click() {
        let folder = sample();
        let mut explorer = open(&folder);
        let at = Instant::now();
        explorer.press_row(0, at);

        explorer.refresh();

        assert_eq!(explorer.press_row(0, at), None);
        assert_eq!(explorer.current(), folder.path());
    }

    // 今のフォルダが消えると、一覧は空になり、エラーが示される。
    #[test]
    fn removed_current_folder_empties_the_list_and_shows_an_error() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.navigate(&folder.path().join("photos"));
        explorer.press_row(0, Instant::now());
        fs::remove_dir_all(folder.path().join("photos")).unwrap();

        explorer.refresh();

        assert!(explorer.entries().is_empty());
        assert_eq!(explorer.selected(), None);
        assert!(explorer.error().is_some());
    }

    // 起動すると、開始フォルダの中身が一覧に並ぶ。
    #[test]
    fn start_folder_items_are_listed_on_open() {
        let folder = sample();

        let explorer = open(&folder);

        assert_eq!(explorer.current(), folder.path());
        assert_eq!(names(&explorer), ["photos", "note.txt"]);
        assert_eq!(explorer.error(), None);
    }

    // フォルダをダブルクリックすると、そのフォルダの中身が一覧に並ぶ。
    #[test]
    fn double_click_on_a_folder_lists_its_items() {
        let folder = sample();
        let mut explorer = open(&folder);

        let opened = double_click(&mut explorer, 0);

        assert_eq!(opened, None);
        assert_eq!(explorer.current(), folder.path().join("photos"));
        assert_eq!(names(&explorer), ["inner"]);
        assert_eq!(explorer.selected(), None);
    }

    // ファイルをダブルクリックすると、開くファイルのパスが返り、フォルダは変わらない。
    #[test]
    fn double_click_on_a_file_returns_its_path_and_keeps_the_folder() {
        let folder = sample();
        let mut explorer = open(&folder);

        let opened = double_click(&mut explorer, 1);

        assert_eq!(opened, Some(folder.path().join("note.txt")));
        assert_eq!(explorer.current(), folder.path());
    }

    // 押した行が、選択された行になる。
    #[test]
    fn pressed_row_becomes_the_selected_row() {
        let folder = sample();
        let mut explorer = open(&folder);

        explorer.press_row(1, Instant::now());

        assert_eq!(explorer.selected(), Some(1));
    }

    // 起動したときは、一覧で表示される。
    #[test]
    fn items_are_shown_as_a_list_on_open() {
        let folder = sample();

        assert_eq!(open(&folder).view_mode(), ViewMode::List);
    }

    // 表示を切り替えると、一覧とグリッドが交互になる。
    #[test]
    fn toggling_the_view_alternates_between_list_and_grid() {
        let folder = sample();
        let mut explorer = open(&folder);

        explorer.toggle_view_mode();
        assert_eq!(explorer.view_mode(), ViewMode::Grid);

        explorer.toggle_view_mode();
        assert_eq!(explorer.view_mode(), ViewMode::List);
    }

    // フォルダを移っても、表示の方法は変わらない。
    #[test]
    fn view_mode_stays_when_the_folder_changes() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_view_mode();

        explorer.navigate(&folder.path().join("photos"));

        assert_eq!(explorer.view_mode(), ViewMode::Grid);
    }

    // 1回押しただけでは、フォルダは変わらない。
    #[test]
    fn single_press_does_not_change_the_folder() {
        let folder = sample();
        let mut explorer = open(&folder);

        let opened = explorer.press_row(0, Instant::now());

        assert_eq!(opened, None);
        assert_eq!(explorer.current(), folder.path());
    }

    // フォルダを移った直後の1回の押下は、ダブルクリックの続きにならない。
    #[test]
    fn first_press_after_entering_a_folder_is_not_part_of_a_double_click() {
        let folder = sample();
        let mut explorer = open(&folder);
        let at = Instant::now();
        explorer.press_row(0, at);
        explorer.navigate(&folder.path().join("photos"));

        explorer.press_row(0, at);

        assert_eq!(explorer.current(), folder.path().join("photos"));
    }

    // 上へ移動すると、親フォルダの中身が一覧に並ぶ。
    #[test]
    fn going_up_lists_the_parent_folder() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.navigate(&folder.path().join("photos"));

        explorer.go_up();

        assert_eq!(explorer.current(), folder.path());
    }

    // パンくずを押すと、そのラベルのフォルダへ移る。
    #[test]
    fn clicking_a_crumb_goes_to_the_folder_of_that_label() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.navigate(&folder.path().join("photos").join("inner"));

        explorer.crumb_clicked("photos");

        assert_eq!(explorer.current(), folder.path().join("photos"));
    }

    // 開けないフォルダへ移ろうとすると、今のフォルダのままエラーが示される。
    #[test]
    fn failed_navigation_keeps_the_folder_and_shows_an_error() {
        let folder = sample();
        let mut explorer = open(&folder);

        explorer.navigate(&folder.path().join("missing"));

        assert_eq!(explorer.current(), folder.path());
        assert_eq!(names(&explorer), ["photos", "note.txt"]);
        assert!(explorer.error().is_some());
    }

    // 移動に成功すると、前のエラーは消える。
    #[test]
    fn successful_navigation_clears_the_earlier_error() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.report_error("failed".to_string());
        assert_eq!(explorer.error(), Some("failed"));

        explorer.navigate(&folder.path().join("photos"));

        assert_eq!(explorer.error(), None);
    }

    // 今のフォルダをお気に入りに追加すると、次の起動でも残っている。
    #[test]
    fn favorite_folder_is_still_there_on_the_next_open() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.navigate(&folder.path().join("photos"));

        explorer.toggle_favorite();

        assert!(explorer.is_current_favorite());
        assert_eq!(open(&folder).favorites(), [folder.path().join("photos")]);
    }

    // お気に入りのフォルダでもう一度切り替えると、お気に入りから外れる。
    #[test]
    fn toggling_again_in_a_favorite_folder_removes_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite();

        explorer.toggle_favorite();

        assert!(!explorer.is_current_favorite());
        assert!(open(&folder).favorites().is_empty());
    }

    // お気に入りを削除すると、次の起動でも消えている。
    #[test]
    fn removed_favorite_is_gone_on_the_next_open() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite();

        explorer.remove_favorite(folder.path());

        assert!(explorer.favorites().is_empty());
        assert!(open(&folder).favorites().is_empty());
    }

    // お気に入りのファイルが壊れていると、お気に入りは空で、エラーが示される。
    #[test]
    fn broken_favorites_file_gives_no_favorites_and_an_error() {
        let folder = sample();
        fs::create_dir(folder.path().join("favorites")).unwrap();
        fs::write(
            folder.path().join("favorites").join("favorites.json"),
            "not json",
        )
        .unwrap();

        let explorer = open(&folder);

        assert!(explorer.favorites().is_empty());
        assert!(explorer.error().is_some());
    }

    // ツリーのフォルダを展開すると、子フォルダだけが下に並ぶ。
    #[test]
    fn expanding_a_tree_folder_shows_only_its_child_folders() {
        let folder = sample();
        let mut explorer = open(&folder);

        explorer.toggle_tree(folder.path());

        let paths: Vec<PathBuf> = explorer
            .tree_rows()
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert_eq!(
            paths,
            [folder.path().to_path_buf(), folder.path().join("photos")]
        );
    }

    // 展開したツリーのフォルダをもう一度切り替えると、折りたたまれる。
    #[test]
    fn toggling_an_expanded_tree_folder_collapses_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_tree(folder.path());

        explorer.toggle_tree(folder.path());

        assert_eq!(explorer.tree_rows().len(), 1);
    }

    // ツリーのフォルダを開くと、一覧がそのフォルダになり、ツリーでも展開される。
    #[test]
    fn opening_a_tree_folder_lists_it_and_expands_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_tree(folder.path());

        explorer.open_tree_folder(&folder.path().join("photos"));

        assert_eq!(explorer.current(), folder.path().join("photos"));
        assert_eq!(explorer.tree_rows().len(), 3);
    }

    // 展開済みのツリーのフォルダを開いても、折りたたまれない。
    #[test]
    fn opening_an_expanded_tree_folder_does_not_collapse_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_tree(folder.path());

        explorer.open_tree_folder(folder.path());

        assert_eq!(explorer.tree_rows().len(), 2);
    }
}
