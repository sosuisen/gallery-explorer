//! The state of the explorer window and the operations the user can do on it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::domain::{
    Arrow, DoubleClick, Entry, Favorites, Selection, SortOrder, Tree, TreeRow, move_selection,
    resolve_crumb, sort_entries,
};
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
    /// The shell thumbnails of files that are not images, by file path. They are encoded images.
    thumbnails: HashMap<PathBuf, Arc<[u8]>>,
    selection: Selection,
    view_mode: ViewMode,
    sort_order: SortOrder,
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
            thumbnails: HashMap::new(),
            selection: Selection::default(),
            view_mode: ViewMode::List,
            sort_order: SortOrder::default(),
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
        self.selection.focus()
    }

    /// The selected rows, in ascending order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection.rows().collect()
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

    pub fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    /// Sorts the items again in `order`. The selected item stays selected.
    pub fn set_sort_order(&mut self, order: SortOrder) {
        self.sort_order = order;
        let mut entries = self.entries.clone();
        sort_entries(&mut entries, order);
        self.replace_entries(entries);
    }

    /// Reads a folder, sorted in the chosen order.
    fn read_sorted(&self, path: &Path) -> std::io::Result<Vec<Entry>> {
        let mut entries = read_folder(path)?;
        sort_entries(&mut entries, self.sort_order);
        Ok(entries)
    }

    /// Shows new items in place of the old ones. The selected item stays selected while it is
    /// still there.
    fn replace_entries(&mut self, entries: Vec<Entry>) {
        let path_of = |row: usize| self.entries.get(row).map(|entry| entry.path.clone());
        let selected: Vec<PathBuf> = self.selection.rows().filter_map(path_of).collect();
        let focus = self.selection.focus().and_then(path_of);
        let row_of = |path: &PathBuf| entries.iter().position(|entry| &entry.path == path);
        // The rows may move, so a press before the change must not pair with one after it.
        self.clicks.reset();
        self.selection = Selection::new(
            selected.iter().filter_map(row_of),
            focus.as_ref().and_then(row_of),
        );
        self.entries = entries;
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

    /// The files that WinUI cannot show as images. The shell may make thumbnails for them.
    pub fn non_image_files(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|entry| !entry.is_folder() && !entry.is_image())
            .map(|entry| entry.path.clone())
            .collect()
    }

    /// Returns the shell thumbnail of a file, when one was made.
    pub fn thumbnail(&self, entry: &Entry) -> Option<&Arc<[u8]>> {
        self.thumbnails.get(&entry.path)
    }

    /// Sets the shell thumbnails made for the files in `folder`.
    /// Thumbnails of a folder that is no longer shown arrive late and are ignored.
    pub fn set_thumbnails(&mut self, folder: &Path, thumbnails: Vec<(PathBuf, Arc<[u8]>)>) {
        if folder == self.current {
            self.thumbnails = thumbnails.into_iter().collect();
        }
    }

    pub fn favorites(&self) -> &[PathBuf] {
        self.favorites.paths()
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        self.favorites.contains(path)
    }

    pub fn tree_rows(&self) -> Vec<TreeRow> {
        self.tree.rows(&self.roots)
    }

    /// Shows the items of `path`. On failure, the current folder stays and the error is kept.
    pub fn navigate(&mut self, path: &Path) {
        match self.read_sorted(path) {
            Ok(entries) => {
                self.current = path.to_path_buf();
                self.entries = entries;
                self.covers.clear();
                self.thumbnails.clear();
                self.selection = Selection::default();
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
        match self.read_sorted(&self.current) {
            Ok(entries) => self.replace_entries(entries),
            Err(error) => {
                self.entries.clear();
                self.selection = Selection::default();
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
        self.selection.select_only(row);
        if !self.clicks.press(row, at) {
            return None;
        }
        self.open_selected()
    }

    /// Adds the pressed row to the selection.
    pub fn ctrl_press_row(&mut self, row: usize) {
        self.selection.add(row);
    }

    /// Moves the selection with a cursor key. columns is the number of tiles in one row of the
    /// grid.
    pub fn move_selection(&mut self, arrow: Arrow, columns: usize) {
        let columns = match self.view_mode {
            ViewMode::List if matches!(arrow, Arrow::Left | Arrow::Right) => return,
            ViewMode::List => 1,
            ViewMode::Grid => columns,
        };
        let moved = move_selection(self.selection.focus(), self.entries.len(), columns, arrow);
        self.selection = Selection::new(moved, moved);
    }

    /// Opens the selected item. A folder is shown. For a file, its path is returned, and the
    /// caller opens it.
    pub fn open_selected(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selection.focus()?)?.clone();
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

    /// Adds a file or a folder to the favorites, or removes it when it is already there.
    pub fn toggle_favorite(&mut self, path: &Path) {
        self.favorites.toggle(path);
        self.save_favorites();
    }

    pub fn remove_favorite(&mut self, path: &Path) {
        self.favorites.remove(path);
        self.save_favorites();
    }

    /// Opens the favorite in `row` of the list of favorites. A folder is shown. For a file, its
    /// path is returned, and the caller opens it.
    /// Favorite folders hold pictures to look at, so they open in the grid, newest first.
    pub fn open_favorite(&mut self, row: usize) -> Option<PathBuf> {
        let path = self.favorites.paths()[row].clone();
        if !path.is_dir() {
            return Some(path);
        }
        // Set before moving, so the new folder is read in this order.
        self.sort_order = SortOrder::ModifiedDescending;
        self.navigate(&path);
        self.view_mode = ViewMode::Grid;
        None
    }

    /// Puts the favorites in the order the user dragged them into, and saves it.
    pub fn reorder_favorites(&mut self, order: Vec<PathBuf>) {
        self.favorites.reorder(order);
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

    // 画像でないファイルの一覧には、フォルダも画像ファイルも含まれない。
    #[test]
    fn non_image_files_leave_out_folders_and_images() {
        let folder = sample();
        fs::write(folder.path().join("a.jpg"), "").unwrap();
        fs::write(folder.path().join("b.clip"), "").unwrap();
        let explorer = open(&folder);

        assert_eq!(
            explorer.non_image_files(),
            [folder.path().join("b.clip"), folder.path().join("note.txt")]
        );
    }

    // 作られたサムネイルが、そのファイルのサムネイルになる。
    #[test]
    fn made_thumbnail_becomes_the_thumbnail_of_its_file() {
        let folder = sample();
        let mut explorer = open(&folder);
        let bytes: Arc<[u8]> = Arc::from(vec![1, 2, 3]);

        explorer.set_thumbnails(
            folder.path(),
            vec![(folder.path().join("note.txt"), bytes.clone())],
        );

        assert_eq!(explorer.thumbnail(&explorer.entries()[1]), Some(&bytes));
        assert_eq!(explorer.thumbnail(&explorer.entries()[0]), None);
    }

    // 前のフォルダのために作ったサムネイルは、無視される。
    #[test]
    fn thumbnails_for_another_folder_are_ignored() {
        let folder = sample();
        let mut explorer = open(&folder);

        explorer.set_thumbnails(
            &folder.path().join("photos"),
            vec![(folder.path().join("note.txt"), Arc::from(vec![1]))],
        );

        assert_eq!(explorer.thumbnail(&explorer.entries()[1]), None);
    }

    // フォルダを移ると、前のフォルダのサムネイルは消える。
    #[test]
    fn thumbnails_are_cleared_when_the_folder_changes() {
        let folder = sample();
        let mut explorer = open(&folder);
        let note = folder.path().join("note.txt");
        explorer.set_thumbnails(folder.path(), vec![(note.clone(), Arc::from(vec![1]))]);

        explorer.navigate(&folder.path().join("photos"));
        explorer.go_up();

        assert_eq!(explorer.thumbnail(&explorer.entries()[1]), None);
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

    // 表示順を変えると、今のフォルダの項目が並び直る。選択していた項目は選択されたままである。
    #[test]
    fn changing_the_sort_order_sorts_the_items_again_and_keeps_the_selection() {
        let folder = sample();
        fs::write(folder.path().join("a.jpg"), "").unwrap();
        let mut explorer = open(&folder);
        explorer.press_row(2, Instant::now());

        explorer.set_sort_order(SortOrder::NameDescending);

        assert_eq!(names(&explorer), ["photos", "note.txt", "a.jpg"]);
        assert_eq!(explorer.selected(), Some(1));
        assert_eq!(explorer.sort_order(), SortOrder::NameDescending);
    }

    // フォルダを移っても、表示順は変わらない。読み直しても変わらない。
    #[test]
    fn sort_order_stays_after_moving_and_refreshing() {
        let folder = sample();
        fs::write(folder.path().join("a.jpg"), "").unwrap();
        let mut explorer = open(&folder);
        explorer.set_sort_order(SortOrder::NameDescending);

        explorer.navigate(&folder.path().join("photos"));
        explorer.go_up();
        assert_eq!(names(&explorer), ["photos", "note.txt", "a.jpg"]);

        fs::write(folder.path().join("b.jpg"), "").unwrap();
        explorer.refresh();
        assert_eq!(names(&explorer), ["photos", "note.txt", "b.jpg", "a.jpg"]);
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

    // Ctrlで押すと、項目が選択に加わる。
    #[test]
    fn ctrl_press_adds_the_item_to_the_selection() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(0, Instant::now());

        explorer.ctrl_press_row(1);

        assert_eq!(explorer.selected_rows(), [0, 1]);
    }

    // 一覧では、グリッドの列の数によらず、下キーで次の行が選ばれる。
    #[test]
    fn down_in_the_list_selects_the_next_row() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(0, Instant::now());

        explorer.move_selection(Arrow::Down, 4);

        assert_eq!(explorer.selected(), Some(1));
    }

    // 選ばれたフォルダを開くと、そのフォルダの中身が並ぶ。
    #[test]
    fn opening_the_selected_folder_lists_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(0, Instant::now());

        let opened = explorer.open_selected();

        assert_eq!(opened, None);
        assert_eq!(explorer.current(), folder.path().join("photos"));
    }

    // 選ばれたファイルを開くと、そのパスが返り、フォルダは変わらない。
    #[test]
    fn opening_the_selected_file_returns_its_path() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(1, Instant::now());

        let opened = explorer.open_selected();

        assert_eq!(opened, Some(folder.path().join("note.txt")));
        assert_eq!(explorer.current(), folder.path());
    }

    // 一覧では、左右キーを押しても、選択は動かない。
    #[test]
    fn left_and_right_in_the_list_keep_the_selection() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.press_row(0, Instant::now());

        explorer.move_selection(Arrow::Right, 1);

        assert_eq!(explorer.selected(), Some(0));
    }

    // グリッドでは、下キーで、1行の列の数だけ後の項目が選ばれる。
    #[test]
    fn down_in_the_grid_selects_the_tile_below() {
        let folder = tempfile::tempdir().unwrap();
        for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            fs::write(folder.path().join(name), "").unwrap();
        }
        let mut explorer = open(&folder);
        explorer.toggle_view_mode();
        explorer.press_row(0, Instant::now());

        explorer.move_selection(Arrow::Down, 2);

        assert_eq!(explorer.selected(), Some(2));
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

    // フォルダをお気に入りに追加すると、次の起動でも残っている。
    #[test]
    fn favorite_folder_is_still_there_on_the_next_open() {
        let folder = sample();
        let photos = folder.path().join("photos");
        let mut explorer = open(&folder);

        explorer.toggle_favorite(&photos);

        assert!(explorer.is_favorite(&photos));
        assert_eq!(open(&folder).favorites(), [photos]);
    }

    // ファイルをお気に入りに追加すると、次の起動でも残っている。
    #[test]
    fn favorite_file_is_still_there_on_the_next_open() {
        let folder = sample();
        let note = folder.path().join("note.txt");
        let mut explorer = open(&folder);

        explorer.toggle_favorite(&note);

        assert!(explorer.is_favorite(&note));
        assert_eq!(open(&folder).favorites(), [note]);
    }

    // お気に入りをもう一度切り替えると、お気に入りから外れる。
    #[test]
    fn toggling_a_favorite_again_removes_it() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());

        explorer.toggle_favorite(folder.path());

        assert!(!explorer.is_favorite(folder.path()));
        assert!(open(&folder).favorites().is_empty());
    }

    // お気に入りを削除すると、次の起動でも消えている。
    #[test]
    fn removed_favorite_is_gone_on_the_next_open() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());

        explorer.remove_favorite(folder.path());

        assert!(explorer.favorites().is_empty());
        assert!(open(&folder).favorites().is_empty());
    }

    // お気に入りの行を選ぶと、その行のフォルダへ移る。
    #[test]
    fn choosing_a_favorite_row_moves_to_its_folder() {
        let folder = sample();
        let photos = folder.path().join("photos");
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());
        explorer.toggle_favorite(&photos);
        explorer.navigate(&photos.join("inner"));

        explorer.open_favorite(1);

        assert_eq!(explorer.current(), photos);
    }

    // お気に入りのファイルを開くと、そのパスが返り、今のフォルダは変わらない。
    #[test]
    fn opening_a_favorite_file_returns_it_and_keeps_the_folder() {
        let folder = sample();
        let photos = folder.path().join("photos");
        let note = folder.path().join("note.txt");
        let mut explorer = open(&folder);
        explorer.toggle_favorite(&note);
        explorer.navigate(&photos);

        let opened = explorer.open_favorite(0);

        assert_eq!(opened, Some(note));
        assert_eq!(explorer.current(), photos);
    }

    // お気に入りのファイルを開いても、表示の方法と表示順は変わらない。
    #[test]
    fn opening_a_favorite_file_keeps_the_view_and_the_order() {
        let folder = sample();
        let note = folder.path().join("note.txt");
        let mut explorer = open(&folder);
        explorer.toggle_favorite(&note);

        explorer.open_favorite(0);

        assert_eq!(explorer.view_mode(), ViewMode::List);
        assert_eq!(explorer.sort_order(), SortOrder::NameAscending);
    }

    // お気に入りのフォルダを開くと、パスは返らない。
    #[test]
    fn opening_a_favorite_folder_returns_nothing() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());

        assert_eq!(explorer.open_favorite(0), None);
    }

    // お気に入りから開くと、一覧で見ていてもグリッドで表示される。
    #[test]
    fn opening_a_favorite_shows_the_grid() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());

        explorer.open_favorite(0);

        assert_eq!(explorer.view_mode(), ViewMode::Grid);
    }

    // お気に入りから開くと、ほかの表示順を選んでいても、更新日の降順で並ぶ。
    #[test]
    fn opening_a_favorite_sorts_newest_first() {
        let folder = sample();
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());

        explorer.open_favorite(0);

        assert_eq!(explorer.sort_order(), SortOrder::ModifiedDescending);
    }

    // お気に入りを並べ替えると、その順序が次の起動でも残っている。
    #[test]
    fn reordered_favorites_keep_their_order_on_the_next_open() {
        let folder = sample();
        let photos = folder.path().join("photos");
        let mut explorer = open(&folder);
        explorer.toggle_favorite(folder.path());
        explorer.toggle_favorite(&photos);

        explorer.reorder_favorites(vec![photos.clone(), folder.path().to_path_buf()]);

        let expected = [photos, folder.path().to_path_buf()];
        assert_eq!(explorer.favorites(), expected);
        assert_eq!(open(&folder).favorites(), expected);
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
