//! The explorer window: it turns the explorer state into WinUI controls.

use std::cell::{Cell, RefCell};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use windows::Win32::Foundation::HWND;
use windows_reactor::*;

use crate::domain::{Arrow, Entry, SortOrder, TreeRow, crumbs, display_name};
use crate::explorer::{Explorer, ViewMode};
use crate::infra::{
    Changes, FavoritesStore, FolderWatcher, collect_thumbnails, drives, folder_covers, image_uri,
    move_to_recycle_bin, open_with_default_app, shell_thumbnail, show_app_icon, show_in_explorer,
};

const SIDE_WIDTH: f64 = 300.0;
const TREE_INDENT: f64 = 16.0;
const TREE_ARROW_WIDTH: f64 = 32.0;
// A gray with low alpha shows on both the light and the dark theme.
const SELECTED_BACKGROUND: Color = Color::argb(0x28, 0x80, 0x80, 0x80);
// The size of one grid cell. The width is the thumbnail plus the tile's margin, border and
// padding. The height also holds the name.
const TILE_WIDTH: f64 = 136.0;
const TILE_HEIGHT: f64 = 132.0;
/// The margin around the list and the grid.
const ITEMS_MARGIN: f64 = 4.0;
const GRID_PREVIEW: PreviewSize = PreviewSize {
    width: 120.0,
    height: 90.0,
    icon_height: 48.0,
    badge_icon_height: 20.0,
    badge_space: 4.0,
};
/// The list shows the same previews at 60% of the size in the grid.
const LIST_PREVIEW: PreviewSize = GRID_PREVIEW.scaled(0.6);
// The longest side of a shell thumbnail in pixels. It stays sharp at a display scale of 200%.
const SHELL_THUMBNAIL_SIZE: u32 = 256;

const SORT_MENU_WIDTH: f64 = 160.0;

/// The rows of the sort menu, in the order they are shown.
const SORT_ORDERS: [(SortOrder, &str); 4] = [
    (SortOrder::NameAscending, "名前の昇順"),
    (SortOrder::NameDescending, "名前の降順"),
    (SortOrder::ModifiedAscending, "更新日の昇順"),
    (SortOrder::ModifiedDescending, "更新日の降順"),
];

#[derive(Clone)]
pub enum Message {
    Up,
    CrumbClicked(String),
    ToggleTree(PathBuf),
    OpenTreeFolder(PathBuf),
    RowPressed(usize, Instant),
    /// A right press on a favorite, a tree row or the empty space of the list. It holds the path
    /// of the file or the folder. Its menu has no delete.
    ShowMenu(PathBuf),
    /// A right press on an item in the list or the grid. It holds the path and whether it is a
    /// folder.
    ShowItemMenu(PathBuf, bool),
    RemoveFavorite(PathBuf),
    /// The selection of the favorites changed. It holds the chosen row.
    FavoriteChosen(Option<usize>),
    /// The user dragged a favorite. These are the keys of the favorites in the new order.
    FavoritesReordered(Vec<String>),
    ToggleViewMode,
    /// An order was chosen in the sort menu. It holds the row of `SORT_ORDERS`.
    SortChosen(Option<usize>),
    /// The watched folder changed.
    FolderChanged(PathBuf),
    /// A wait for changes ended without a change.
    WatchEnded,
    /// The first images of the folders in a folder were found.
    CoversFound(PathBuf, Vec<(PathBuf, PathBuf)>),
    /// The shell thumbnails of the files in a folder were made.
    ThumbnailsMade(PathBuf, Vec<(PathBuf, Arc<[u8]>)>),
    /// A key for the items was pressed.
    KeyPressed(Key),
    /// The client area of the window changed. It holds the width.
    WindowResized(f64),
}

/// A key that works on the items of the current folder.
#[derive(Clone, Copy)]
pub enum Key {
    Arrow(Arrow),
    Enter,
    /// Backspace, which goes up to the parent folder.
    Back,
}

pub struct ExplorerWindow {
    explorer: Explorer,
    watch: Option<Watch>,
    // The task that looks for the covers of the folders in the current folder.
    covers_task: Option<ComponentTask>,
    // The task that asks the shell for the thumbnails of the files in the current folder.
    thumbnails_task: Option<ComponentTask>,
    /// The number of tiles in one row of the grid. It follows the width of the window.
    columns: usize,
    /// The first hidden button in the selected item. See `with_scroll_buttons`.
    focus_anchor: ElementRef<Button>,
    /// Counts the moves of the selection by key. Each new count focuses the selected item.
    key_moves: u64,
}

/// The watch of the current folder and the background task that waits for its changes.
struct Watch {
    folder: PathBuf,
    // None when the folder cannot be watched. The list then works without live updates.
    watcher: Option<FolderWatcher>,
    task: Option<ComponentTask>,
}

impl Component for ExplorerWindow {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let roots = drives();
        let start = env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| roots.first().cloned())
            .unwrap_or_default();
        let mut window = Self {
            explorer: Explorer::open(roots, &start, FavoritesStore::in_app_data()),
            watch: None,
            covers_task: None,
            thumbnails_task: None,
            columns: 1,
            focus_anchor: ElementRef::new(),
            key_moves: 0,
        };
        window.watch_current_folder(context);
        if let Err(error) = hook_keys(context.sender()) {
            window
                .explorer
                .report_error(format!("キーで項目を選べません: {error}"));
        }
        window
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        self.handle(message, context);
        self.watch_current_folder(context);
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title(format!(
            "{} - GalleryExplorer",
            display_name(self.explorer.current())
        ));
        context.window_visuals(WindowVisuals::new().client_size(1100.0, 700.0));
        context.on_window_size(
            context.callback(|size: WindowSize| Message::WindowResized(size.width)),
        );
        // The dependency never changes, so this runs once, after the window exists.
        context.use_effect("app-icon", (), || {
            show_app_icon();
            None
        });
        // The effect runs after the new view is published, so the anchor is already in the
        // newly selected item.
        let anchor = self.focus_anchor.clone();
        context.use_effect("scroll-to-selected", self.key_moves, move || {
            let _ = anchor.request_focus_result(|focused| {
                if focused == Ok(true) {
                    press_tab_soon();
                }
            });
            None
        });

        Grid::new()
            .rows([GridLength::STAR, GridLength::Auto])
            .columns([GridLength::Pixel(SIDE_WIDTH), GridLength::STAR])
            .children((
                self.side_pane(context),
                self.file_pane(context),
                self.status_bar(),
            ))
    }
}

impl ExplorerWindow {
    fn handle(&mut self, message: Message, context: &ComponentContext<Self>) {
        let explorer = &mut self.explorer;
        match message {
            Message::Up | Message::KeyPressed(Key::Back) => explorer.go_up(),
            Message::CrumbClicked(label) => explorer.crumb_clicked(&label),
            Message::ToggleTree(path) => explorer.toggle_tree(&path),
            Message::OpenTreeFolder(path) => explorer.open_tree_folder(&path),
            Message::RowPressed(row, at) => {
                if let Some(file) = explorer.press_row(row, at) {
                    open_file(explorer, &file);
                }
            }
            Message::KeyPressed(Key::Arrow(arrow)) => {
                explorer.move_selection(arrow, self.columns);
                self.key_moves += 1;
            }
            Message::KeyPressed(Key::Enter) => {
                if let Some(file) = explorer.open_selected() {
                    open_file(explorer, &file);
                }
            }
            Message::WindowResized(width) => self.columns = grid_columns(width),
            Message::ShowMenu(path) => {
                let is_folder = path.is_dir();
                show_context_menu(explorer, &path, is_folder, false);
            }
            Message::ShowItemMenu(path, is_folder) => {
                show_context_menu(explorer, &path, is_folder, true);
            }
            Message::RemoveFavorite(path) => explorer.remove_favorite(&path),
            // The view clears the selection again, so the same favorite can be chosen twice.
            Message::FavoriteChosen(Some(row)) => match explorer.open_favorite(row) {
                Some(file) => open_file(explorer, &file),
                // The favorite may be the current folder. Moving there clears the covers, but
                // no folder change loads them again, nor the thumbnails that the grid now needs.
                None => self.load_previews(context),
            },
            Message::FavoriteChosen(None) => {}
            Message::FavoritesReordered(keys) => {
                explorer.reorder_favorites(keys.into_iter().map(PathBuf::from).collect())
            }
            Message::ToggleViewMode => explorer.toggle_view_mode(),
            Message::SortChosen(Some(row)) => explorer.set_sort_order(SORT_ORDERS[row].0),
            Message::SortChosen(None) => {}
            Message::FolderChanged(folder) => {
                // A change of a folder that is no longer shown arrives late and is ignored.
                let is_current = folder == explorer.current();
                if is_current {
                    explorer.refresh();
                }
                if let Some(watch) = self.watch.as_mut().filter(|watch| watch.folder == folder)
                    && let Some(watcher) = &watch.watcher
                {
                    watch.task = Some(wait_for_changes(watcher.changes(), folder, context));
                }
                if is_current {
                    self.load_previews(context);
                }
            }
            Message::WatchEnded => {}
            Message::CoversFound(folder, covers) => explorer.set_covers(&folder, covers),
            Message::ThumbnailsMade(folder, thumbnails) => {
                explorer.set_thumbnails(&folder, thumbnails)
            }
        }
    }

    fn load_previews(&mut self, context: &ComponentContext<Self>) {
        self.load_covers(context);
        self.load_thumbnails(context);
    }

    /// Asks the shell for the thumbnails of the files that are not images, on the thread pool.
    fn load_thumbnails(&mut self, context: &ComponentContext<Self>) {
        if let Some(task) = self.thumbnails_task.take() {
            task.cancel();
        }
        let folder = self.explorer.current().to_path_buf();
        let files = self.explorer.non_image_files();
        self.thumbnails_task = Some(context.spawn_background(move |token| {
            let thumbnails = collect_thumbnails(
                files,
                || token.is_cancelled(),
                |file| shell_thumbnail(file, SHELL_THUMBNAIL_SIZE),
            );
            Message::ThumbnailsMade(folder, thumbnails)
        }));
    }

    /// Looks for the covers of the folders in the current folder on the thread pool, because
    /// each folder must be read. A search for an earlier folder is cancelled.
    fn load_covers(&mut self, context: &ComponentContext<Self>) {
        if let Some(task) = self.covers_task.take() {
            task.cancel();
        }
        let folder = self.explorer.current().to_path_buf();
        let folders = self.explorer.folders();
        self.covers_task = Some(context.spawn_background(move |token| {
            let covers = folder_covers(folders, || token.is_cancelled());
            Message::CoversFound(folder, covers)
        }));
    }

    /// Watches the current folder when it is not watched yet, and stops watching the old one.
    fn watch_current_folder(&mut self, context: &ComponentContext<Self>) {
        let current = self.explorer.current();
        if self
            .watch
            .as_ref()
            .is_some_and(|watch| watch.folder == current)
        {
            return;
        }
        if let Some(task) = self.watch.take().and_then(|watch| watch.task) {
            task.cancel();
        }
        let watcher = FolderWatcher::start(current).ok();
        let task = watcher
            .as_ref()
            .map(|watcher| wait_for_changes(watcher.changes(), current.to_path_buf(), context));
        self.watch = Some(Watch {
            folder: current.to_path_buf(),
            watcher,
            task,
        });
        self.load_previews(context);
    }

    fn path_bar(&self, context: &ViewContext<Self>) -> View {
        let explorer = &self.explorer;
        let labels = crumbs(explorer.current())
            .into_iter()
            .map(|crumb| crumb.label);

        Grid::new()
            .grid_row(0)
            .margin(8.0)
            .column_spacing(8.0)
            .columns([GridLength::Auto, GridLength::STAR])
            .children((
                Button::new()
                    .grid_column(0)
                    .is_enabled(explorer.current().parent().is_some())
                    .on_click(context.message(Message::Up))
                    .content(icon_label(Symbol::Up, "上へ")),
                BreadcrumbBar::new()
                    .grid_column(1)
                    .vertical_alignment(VerticalAlignment::Center)
                    .items_source(labels)
                    .on_item_clicked(context.callback(Message::CrumbClicked)),
            ))
    }

    /// The right pane: the path of the current folder, a toolbar for how the items are shown,
    /// and the items.
    fn file_pane(&self, context: &ViewContext<Self>) -> View {
        Grid::new()
            .grid_row(0)
            .grid_column(1)
            .rows([GridLength::Auto, GridLength::Auto, GridLength::STAR])
            .children((
                self.path_bar(context),
                self.file_toolbar(context),
                self.file_list(context),
            ))
    }

    fn file_toolbar(&self, context: &ViewContext<Self>) -> View {
        let explorer = &self.explorer;
        // The button names the view that it switches to.
        let (view_icon, view_label) = match explorer.view_mode() {
            ViewMode::List => (Symbol::ViewAll, "グリッド"),
            ViewMode::Grid => (Symbol::List, "一覧"),
        };
        let sort_row = SORT_ORDERS
            .iter()
            .position(|(order, _)| *order == explorer.sort_order());

        StackPanel::new()
            .grid_row(1)
            .orientation(Orientation::Horizontal)
            // The path bar above already leaves a margin.
            .margin(Thickness::new(8.0, 0.0, 8.0, 8.0))
            .spacing(8.0)
            .children((
                Button::new()
                    .on_click(context.message(Message::ToggleViewMode))
                    .content(icon_label(view_icon, view_label)),
                ComboBox::new()
                    // A fixed width keeps the menu the same size when the chosen label changes.
                    .width(SORT_MENU_WIDTH)
                    .vertical_alignment(VerticalAlignment::Center)
                    .items_source(SORT_ORDERS.iter().map(|(_, label)| *label))
                    .selected_index(sort_row)
                    .on_selection_changed(context.callback(Message::SortChosen)),
            ))
    }

    fn side_pane(&self, context: &ViewContext<Self>) -> View {
        let favorites = self
            .explorer
            .favorites()
            .iter()
            .map(|path| {
                // ListView reports the new order by the tags of its items.
                let item = ListViewItem::new()
                    .tag(key(path))
                    .content(favorite_row(path, context));
                KeyedView::new(key(path), item)
            })
            .collect::<Vec<_>>();
        let tree = self
            .explorer
            .tree_rows()
            .into_iter()
            .map(|row| (key(&row.path), tree_row(&row, context)));

        ScrollViewer::new().grid_row(0).grid_column(0).content(
            StackPanel::new().margin(8.0).spacing(4.0).children((
                heading("お気に入り"),
                // Unlike the file list, the favorites use a ListView for its drag to reorder.
                // Its animation does not matter here, because the favorites rarely change.
                // A Button in a row would take the press, and the drag would never start.
                // So a row is chosen by selection, which the view clears every time.
                ListView::new()
                    .selection_mode(ListViewSelectionMode::Single)
                    .selected_index(None)
                    .on_selection_changed(context.callback(Message::FavoriteChosen))
                    .can_drag_items(true)
                    .can_reorder_items(true)
                    .allow_drop(true)
                    .on_reordered(context.callback(Message::FavoritesReordered))
                    .collection_slot(ListViewSlot::Items, favorites),
                heading("フォルダ"),
                StackPanel::new().keyed_children(tree),
            )),
        )
    }

    // A ListView animates its items when they change, and windows-reactor cannot turn that off.
    // A StackPanel has no animation, so a folder change shows the new items at once.
    // An ItemsRepeater has no animation either, but windows-reactor puts each of its rows in a
    // left-aligned container, so a row cannot fill the width.
    // The grid uses a VariableSizedWrapGrid instead of a GridView for the same reason.
    fn file_list(&self, context: &ViewContext<Self>) -> View {
        let selected = self.explorer.focus();
        let mode = self.explorer.view_mode();
        let items = self
            .explorer
            .entries()
            .iter()
            .enumerate()
            .map(|(row, entry)| {
                let is_selected = selected == Some(row);
                let source = thumbnail_source(&self.explorer, entry);
                let view = match mode {
                    ViewMode::List => file_row(row, entry, source, is_selected, context),
                    ViewMode::Grid => file_tile(row, entry, source, is_selected, context),
                };
                let view = with_scroll_buttons(view, is_selected.then_some(&self.focus_anchor));
                (entry.name.clone(), view)
            });
        let panel: View = match mode {
            ViewMode::List => StackPanel::new().margin(ITEMS_MARGIN).keyed_children(items),
            ViewMode::Grid => VariableSizedWrapGrid::new()
                .orientation(Orientation::Horizontal)
                .item_width(TILE_WIDTH)
                .item_height(TILE_HEIGHT)
                .margin(ITEMS_MARGIN)
                .keyed_children(items),
        };

        // The border takes the right presses on the empty space around the items.
        Border::new()
            .grid_row(2)
            // A border without a background receives no pointer events.
            .background(Color::transparent())
            .on_pointer_pressed(background_menu_handler(self.explorer.current(), context))
            .content(ScrollViewer::new().content(panel))
    }

    fn status_bar(&self) -> View {
        let explorer = &self.explorer;
        let text = match explorer.error() {
            Some(error) => TextBlock::new()
                .text(error)
                .foreground(ThemeBrush::SystemCritical),
            None => TextBlock::new().text(format!("{}個の項目", explorer.entries().len())),
        };
        text.grid_row(1).grid_column_span(2).margin(8.0).into()
    }
}

fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn heading(text: &str) -> TextBlock {
    TextBlock::new()
        .text(text)
        .font_weight(FontWeight::SEMI_BOLD)
        .margin(Thickness::new(4.0, 8.0, 0.0, 0.0))
}

fn icon_label(symbol: Symbol, text: impl Into<String>) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(8.0)
        .children((
            SymbolIcon::new().symbol(symbol),
            TextBlock::new()
                .text(text)
                .vertical_alignment(VerticalAlignment::Center),
        ))
}

fn favorite_row(path: &Path, context: &ViewContext<ExplorerWindow>) -> View {
    Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .children((
            Border::new()
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Stretch)
                // A border without a background receives no pointer events.
                .background(Color::transparent())
                .on_pointer_pressed(menu_handler(path, context))
                .content(icon_label(Symbol::SolidStar, display_name(path))),
            Button::new()
                .grid_column(1)
                .style(ButtonStyle::Subtle)
                .on_click(context.message(Message::RemoveFavorite(path.to_path_buf())))
                .content(SymbolIcon::new().symbol(Symbol::Delete)),
        ))
}

fn tree_row(row: &TreeRow, context: &ViewContext<ExplorerWindow>) -> View {
    let indent = TREE_INDENT * row.depth as f64;
    let arrow = if row.expanded { "▼" } else { "▶" };

    // The folder button fills the whole row, so a click anywhere opens the folder.
    // The arrow button lies on top of it, in the place the indent gives it.
    // A button reports no pointer events, so the border around the row takes the right press.
    let row_view = Grid::new().children((
        Button::new()
            .style(ButtonStyle::Subtle)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .horizontal_content_alignment(HorizontalAlignment::Left)
            .on_click(context.message(Message::OpenTreeFolder(row.path.clone())))
            .content(
                Border::new()
                    .margin(Thickness::new(indent + TREE_ARROW_WIDTH, 0.0, 0.0, 0.0))
                    .content(icon_label(Symbol::Folder, display_name(&row.path))),
            ),
        Button::new()
            .style(ButtonStyle::Subtle)
            .width(TREE_ARROW_WIDTH)
            .margin(Thickness::new(indent, 0.0, 0.0, 0.0))
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Stretch)
            .on_click(context.message(Message::ToggleTree(row.path.clone())))
            .content(TextBlock::new().text(arrow).font_size(10.0)),
    ));
    Border::new()
        .on_pointer_pressed(menu_handler(&row.path, context))
        .content(row_view)
}

/// The items of the context menus.
const OPEN_IN_EXPLORER: &str = "エクスプローラーで開く";
const ADD_FAVORITE: &str = "お気に入りに追加";
const REMOVE_FAVORITE: &str = "お気に入りから外す";
const DELETE: &str = "削除";

/// Shows the context menu of a file or a folder, and does what the user chose. Only the items
/// in the list and the grid can be deleted.
fn show_context_menu(explorer: &mut Explorer, path: &Path, is_folder: bool, can_delete: bool) {
    let favorite = if explorer.is_favorite(path) {
        REMOVE_FAVORITE
    } else {
        ADD_FAVORITE
    };
    let items: &[&str] = if can_delete {
        &[OPEN_IN_EXPLORER, favorite, DELETE]
    } else {
        &[OPEN_IN_EXPLORER, favorite]
    };
    match popup_menu(items) {
        Some(0) => open_in_explorer(explorer, path, is_folder),
        Some(1) => explorer.toggle_favorite(path),
        Some(2) => {
            // The folder watch removes the item from the list.
            if let Err(error) = move_to_recycle_bin(path) {
                explorer.report_error(format!("{}を削除できません: {error}", path.display()));
            }
        }
        _ => {}
    }
}

fn open_in_explorer(explorer: &mut Explorer, path: &Path, is_folder: bool) {
    if let Err(error) = show_in_explorer(path, is_folder) {
        explorer.report_error(format!("{}を開けません: {error}", path.display()));
    }
}

fn open_file(explorer: &mut Explorer, file: &Path) {
    if let Err(error) = open_with_default_app(file) {
        explorer.report_error(format!("{}を開けません: {error}", file.display()));
    }
}

/// The number of tiles in one row of the grid, for a window whose client area is `width` wide.
///
/// The grid gets the width beside the side pane, less its margin on both sides. The scroll bar
/// lies over the tiles, so it takes no width.
fn grid_columns(width: f64) -> usize {
    let room = width - SIDE_WIDTH - ITEMS_MARGIN * 2.0;
    ((room / TILE_WIDTH).floor() as usize).max(1)
}

/// Puts two hidden buttons over the left edge of the selected item, so that a key can scroll the
/// item into view.
///
/// windows-reactor 0.100.0 cannot scroll a ScrollViewer. WinUI scrolls an element into view only
/// when it gets keyboard focus, not when the app focuses it. So the view focuses the first
/// button, the anchor, and then presses Tab. Tab moves keyboard focus to the second button, and
/// WinUI scrolls the item into view.
///
/// Both buttons are in the same item. A posted key cannot hold Shift, so focus can only move
/// forward. The buttons are 1px wide and transparent, so they hardly take any clicks from the
/// item.
///
/// Every item is in the grid, and only the selected one has the buttons. A press selects the
/// item. If only the selected item were in a grid, the press would replace the item's control
/// with a new one, and the drag of the item could not start.
fn with_scroll_buttons(item: View, anchor: Option<&ElementRef<Button>>) -> View {
    let (first, second): (View, View) = match anchor {
        Some(anchor) => (
            scroll_button().element_ref(anchor).into(),
            scroll_button().into(),
        ),
        None => (View::empty(), View::empty()),
    };
    Grid::new().children((item, first, second))
}

fn scroll_button() -> Button {
    Button::new()
        .opacity(0.0)
        .width(1.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch)
}

fn file_row(
    row: usize,
    entry: &Entry,
    source: Option<ThumbnailSource>,
    selected: bool,
    context: &ViewContext<ExplorerWindow>,
) -> View {
    // The selected row gets a gray background and an accent bar on the left. Every row keeps the
    // bar's width, so the text does not move when the selection changes.
    let (background, bar) = selection_brushes(selected);

    Border::new()
        .background(background)
        .border_brush(bar)
        .border_thickness(Thickness::new(3.0, 0.0, 0.0, 0.0))
        .corner_radius(4.0)
        .padding(Thickness::new(12.0, 4.0, 12.0, 4.0))
        .on_pointer_pressed(press_handler(row, entry, context))
        .drag_storage_items(DragStorageItems::new([entry.path.clone()]))
        .content(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .children((
                    preview(entry, source, LIST_PREVIEW),
                    TextBlock::new()
                        .text(entry.name.clone())
                        .vertical_alignment(VerticalAlignment::Center),
                )),
        )
}

fn file_tile(
    row: usize,
    entry: &Entry,
    source: Option<ThumbnailSource>,
    selected: bool,
    context: &ViewContext<ExplorerWindow>,
) -> View {
    // The selected tile gets a gray background and an accent outline.
    let (background, outline) = selection_brushes(selected);

    Border::new()
        .background(background)
        .border_brush(outline)
        .border_thickness(2.0)
        .corner_radius(6.0)
        .margin(2.0)
        .padding(4.0)
        .on_pointer_pressed(press_handler(row, entry, context))
        .drag_storage_items(DragStorageItems::new([entry.path.clone()]))
        .content(
            StackPanel::new().spacing(4.0).children((
                preview(entry, source, GRID_PREVIEW),
                TextBlock::new()
                    .text(entry.name.clone())
                    .text_trimming(TextTrimming::CharacterEllipsis)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )),
        )
}

/// Where the thumbnail of a tile comes from.
enum ThumbnailSource {
    /// An image file that WinUI reads itself.
    File(PathBuf),
    /// An image that the shell made, encoded as a BMP file.
    Encoded(Arc<[u8]>),
}

/// An image file shows itself, and a folder shows its first image. Any other file shows the
/// thumbnail that the shell made, when there is one.
fn thumbnail_source(explorer: &Explorer, entry: &Entry) -> Option<ThumbnailSource> {
    if entry.is_image() {
        Some(ThumbnailSource::File(entry.path.clone()))
    } else if let Some(cover) = explorer.cover(entry) {
        Some(ThumbnailSource::File(cover.to_path_buf()))
    } else {
        explorer
            .thumbnail(entry)
            .map(|bytes| ThumbnailSource::Encoded(bytes.clone()))
    }
}

/// The size of a preview, and of the folder badge on a cover.
#[derive(Clone, Copy)]
struct PreviewSize {
    width: f64,
    height: f64,
    icon_height: f64,
    badge_icon_height: f64,
    /// The padding and the margin of the badge.
    badge_space: f64,
}

impl PreviewSize {
    const fn scaled(self, factor: f64) -> Self {
        Self {
            width: self.width * factor,
            height: self.height * factor,
            icon_height: self.icon_height * factor,
            badge_icon_height: self.badge_icon_height * factor,
            badge_space: self.badge_space * factor,
        }
    }
}

/// Shows the thumbnail, or a large icon when there is none.
fn preview(entry: &Entry, source: Option<ThumbnailSource>, size: PreviewSize) -> View {
    let has_thumbnail = source.is_some();
    let thumbnail = source.and_then(|source| match source {
        // Not `source_file`: `image_uri` says why.
        ThumbnailSource::File(path) => Image::new().source(image_uri(&path)).ok(),
        // The same shared bytes on every render let WinUI keep the decoded image.
        ThumbnailSource::Encoded(bytes) => Some(Image::new().source_data(EncodedImage::new(bytes))),
    });
    let content: View = match thumbnail {
        Some(image) => image
            .stretch(Stretch::Uniform)
            .width(size.width)
            .height(size.height)
            .into(),
        None => Viewbox::new().height(size.icon_height).slot(
            ViewboxSlot::Child,
            SymbolIcon::new().symbol(entry_symbol(entry)),
        ),
    };
    let grid = Grid::new().width(size.width).height(size.height);
    if entry.is_folder() && has_thumbnail {
        grid.children((content, folder_badge(size)))
    } else {
        grid.children((content,))
    }
}

/// A small folder icon on the cover, so that a folder does not look like an image file.
fn folder_badge(size: PreviewSize) -> View {
    Border::new()
        .background(Brush::Theme(ThemeBrush::SolidBackground))
        .corner_radius(4.0)
        .padding(size.badge_space)
        .margin(size.badge_space)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Bottom)
        .content(
            Viewbox::new()
                .height(size.badge_icon_height)
                .slot(ViewboxSlot::Child, SymbolIcon::new().symbol(Symbol::Folder)),
        )
}

/// Waits on the thread pool, because the watcher's thread cannot send to the window.
fn wait_for_changes(
    changes: Changes,
    folder: PathBuf,
    context: &ComponentContext<ExplorerWindow>,
) -> ComponentTask {
    context.spawn_background(move |token| {
        if changes.wait(|| token.is_cancelled()) {
            Message::FolderChanged(folder)
        } else {
            Message::WatchEnded
        }
    })
}

fn entry_symbol(entry: &Entry) -> Symbol {
    if entry.is_folder() {
        Symbol::Folder
    } else {
        Symbol::Document
    }
}

/// Returns the background and the border brush of a row or a tile.
fn selection_brushes(selected: bool) -> (Color, Brush) {
    if selected {
        (SELECTED_BACKGROUND, Brush::Theme(ThemeBrush::Accent))
    } else {
        // A border without a background receives no pointer events.
        (Color::transparent(), Brush::Solid(Color::transparent()))
    }
}

/// The items report no double click, so each item reports every left press with its time.
/// A right press asks for the context menu of the item.
fn press_handler(
    row: usize,
    entry: &Entry,
    context: &ViewContext<ExplorerWindow>,
) -> impl Fn(PointerEventInfo) + use<> {
    let sender = context.sender();
    let path = entry.path.clone();
    let is_folder = entry.is_folder();
    move |pointer: PointerEventInfo| {
        if pointer.is_left_button_pressed {
            sender.send(Message::RowPressed(row, Instant::now()));
        } else if pointer.is_right_button_pressed {
            ITEM_MENU_ASKED.set(true);
            sender.send(Message::ShowItemMenu(path.clone(), is_folder));
        }
    }
}

/// A right press on the empty space of the list or the grid asks for the context menu of the
/// current folder.
///
/// A press on an item also reaches this handler, after the item's handler, because WinUI passes
/// the press on to the parents. The item's handler marks the press, and this handler ignores it.
fn background_menu_handler(
    folder: &Path,
    context: &ViewContext<ExplorerWindow>,
) -> impl Fn(PointerEventInfo) + use<> {
    let sender = context.sender();
    let folder = folder.to_path_buf();
    move |pointer: PointerEventInfo| {
        if pointer.is_right_button_pressed && !ITEM_MENU_ASKED.replace(false) {
            sender.send(Message::ShowMenu(folder.clone()));
        }
    }
}

/// A right press on a favorite or a tree row asks for the context menu of its file or folder.
fn menu_handler(
    path: &Path,
    context: &ViewContext<ExplorerWindow>,
) -> impl Fn(PointerEventInfo) + use<> {
    let sender = context.sender();
    let path = path.to_path_buf();
    move |pointer: PointerEventInfo| {
        if pointer.is_right_button_pressed {
            sender.send(Message::ShowMenu(path.clone()));
        }
    }
}

/// Shows a menu at the mouse cursor and returns the index of the chosen item.
///
/// windows-reactor 0.100.0 has no `MenuFlyout`, so this is a plain Win32 popup menu. It runs
/// its own message loop until the user chooses an item or closes the menu.
fn popup_menu(items: &[&str]) -> Option<usize> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_STRING, SetForegroundWindow,
        TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    };
    use windows::core::HSTRING;

    unsafe {
        let menu = CreatePopupMenu().ok()?;
        // The item ids start at 1, because TrackPopupMenu returns 0 when nothing is chosen.
        for (index, item) in items.iter().enumerate() {
            AppendMenuW(menu, MF_STRING, index + 1, &HSTRING::from(*item)).ok()?;
        }
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).ok()?;
        let window = GetActiveWindow();
        // Without this, the menu does not close when the user clicks outside of it.
        _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            None,
            window,
            None,
        );
        _ = DestroyMenu(menu);
        (chosen.0 as usize).checked_sub(1)
    }
}

thread_local! {
    /// Whether the right press that is being handled was on an item. See
    /// `background_menu_handler`.
    static ITEM_MENU_ASKED: Cell<bool> = const { Cell::new(false) };
    /// Where the key hook sends the keys. The hook runs on the UI thread, as the window does.
    static KEY_SENDER: RefCell<Option<LocalSender<Message>>> = const { RefCell::new(None) };
    /// The window that WinUI reads the keys from. It is the window of the last key.
    static KEY_WINDOW: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
}

/// How long the Tab of `with_scroll_buttons` waits. The buttons are new, and WinUI lays them out
/// only after the view is published. A Tab before that scrolled to the item that was selected
/// before.
const TAB_DELAY_MS: u32 = 50;

/// Presses Tab after `TAB_DELAY_MS`.
fn press_tab_soon() {
    use windows::Win32::UI::WindowsAndMessaging::SetTimer;

    // A timer without a window calls its procedure on this thread.
    unsafe { SetTimer(None, 0, TAB_DELAY_MS, Some(tab_timer)) };
}

unsafe extern "system" fn tab_timer(_window: HWND, _message: u32, timer: usize, _time: u32) {
    use windows::Win32::UI::WindowsAndMessaging::KillTimer;

    let _ = unsafe { KillTimer(None, timer) };
    press_tab();
}

/// Presses Tab in the window, as if on the keyboard. The posted key does not go through the
/// keyboard, so it cannot reach another app.
fn press_tab() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB;
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_KEYDOWN, WM_KEYUP};

    let window = KEY_WINDOW.get();
    if window.is_invalid() {
        return;
    }
    let key = WPARAM(usize::from(VK_TAB.0));
    // The lowest bits are the repeat count. The highest two bits mark a key that goes up.
    let _ = unsafe { PostMessageW(Some(window), WM_KEYDOWN, key, LPARAM(1)) };
    let _ = unsafe { PostMessageW(Some(window), WM_KEYUP, key, LPARAM(0xC000_0001)) };
}

/// Sends the cursor keys and Enter to the window as `Message::KeyPressed`.
///
/// windows-reactor 0.100.0 has no key events, and its key accelerators do not cover the cursor
/// keys. A `WH_GETMESSAGE` hook on the UI thread sees each key before WinUI does. The hook stays
/// until the app ends.
fn hook_keys(sender: LocalSender<Message>) -> windows::core::Result<()> {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_GETMESSAGE};

    KEY_SENDER.with(|slot| *slot.borrow_mut() = Some(sender));
    unsafe { SetWindowsHookExW(WH_GETMESSAGE, Some(key_hook), None, GetCurrentThreadId()) }?;
    Ok(())
}

unsafe extern "system" fn key_hook(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, MSG, PM_REMOVE, WM_KEYDOWN, WM_NULL,
    };

    // A message that is only peeked at comes again when it is removed.
    if code == HC_ACTION as i32 && wparam.0 == PM_REMOVE.0 as usize {
        // SAFETY: for WH_GETMESSAGE, lparam points to the message being taken from the queue.
        let message = unsafe { &mut *(lparam.0 as *mut MSG) };
        if message.message == WM_KEYDOWN
            && let Some(key) = key_of(message.wParam.0)
        {
            KEY_WINDOW.set(message.hwnd);
            let sent = KEY_SENDER.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .is_some_and(|sender| sender.send(Message::KeyPressed(key)))
            });
            // WinUI must not act on the key too. A focused button would be pressed by Enter, and
            // the cursor keys would move the focus away from the items.
            if sent {
                message.message = WM_NULL;
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn key_of(virtual_key: usize) -> Option<Key> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_BACK, VK_DOWN, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP,
    };

    let key = u16::try_from(virtual_key).ok()?;
    Some(match key {
        _ if key == VK_UP.0 => Key::Arrow(Arrow::Up),
        _ if key == VK_DOWN.0 => Key::Arrow(Arrow::Down),
        _ if key == VK_LEFT.0 => Key::Arrow(Arrow::Left),
        _ if key == VK_RIGHT.0 => Key::Arrow(Arrow::Right),
        _ if key == VK_RETURN.0 => Key::Enter,
        _ if key == VK_BACK.0 => Key::Back,
        _ => return None,
    })
}
