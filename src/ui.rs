//! The explorer window: it turns the explorer state into WinUI controls.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use windows_reactor::*;

use crate::domain::{Entry, TreeRow, crumbs, display_name};
use crate::explorer::{Explorer, ViewMode};
use crate::infra::{
    Changes, FavoritesStore, FolderWatcher, collect_thumbnails, drives, folder_covers, image_uri,
    open_with_default_app, shell_thumbnail, show_in_explorer,
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
const THUMBNAIL_WIDTH: f64 = 120.0;
const THUMBNAIL_HEIGHT: f64 = 90.0;
const GRID_ICON_HEIGHT: f64 = 48.0;
// The longest side of a shell thumbnail in pixels. It stays sharp at a display scale of 200%.
const SHELL_THUMBNAIL_SIZE: u32 = 256;

#[derive(Clone)]
pub enum Message {
    Up,
    CrumbClicked(String),
    ToggleTree(PathBuf),
    OpenTreeFolder(PathBuf),
    RowPressed(usize, Instant),
    /// A right press on an item. It holds the path and whether it is a folder.
    ShowMenu(PathBuf, bool),
    ToggleFavorite,
    RemoveFavorite(PathBuf),
    /// The selection of the favorites changed. It holds the chosen row.
    FavoriteChosen(Option<usize>),
    /// The user dragged a favorite. These are the keys of the favorites in the new order.
    FavoritesReordered(Vec<String>),
    ToggleViewMode,
    /// The watched folder changed.
    FolderChanged(PathBuf),
    /// A wait for changes ended without a change.
    WatchEnded,
    /// The first images of the folders in a folder were found.
    CoversFound(PathBuf, Vec<(PathBuf, PathBuf)>),
    /// The shell thumbnails of the files in a folder were made.
    ThumbnailsMade(PathBuf, Vec<(PathBuf, Arc<[u8]>)>),
}

pub struct ExplorerWindow {
    explorer: Explorer,
    watch: Option<Watch>,
    // The task that looks for the covers of the folders in the current folder.
    covers_task: Option<ComponentTask>,
    // The task that asks the shell for the thumbnails of the files in the current folder.
    thumbnails_task: Option<ComponentTask>,
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
        };
        window.watch_current_folder(context);
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

        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR, GridLength::Auto])
            .columns([GridLength::Pixel(SIDE_WIDTH), GridLength::STAR])
            .children((
                self.toolbar(context),
                self.side_pane(context),
                self.file_list(context),
                self.status_bar(),
            ))
    }
}

impl ExplorerWindow {
    fn handle(&mut self, message: Message, context: &ComponentContext<Self>) {
        let explorer = &mut self.explorer;
        match message {
            Message::Up => explorer.go_up(),
            Message::CrumbClicked(label) => explorer.crumb_clicked(&label),
            Message::ToggleTree(path) => explorer.toggle_tree(&path),
            Message::OpenTreeFolder(path) => explorer.open_tree_folder(&path),
            Message::RowPressed(row, at) => {
                if let Some(file) = explorer.press_row(row, at)
                    && let Err(error) = open_with_default_app(&file)
                {
                    explorer.report_error(format!("{}を開けません: {error}", file.display()));
                }
            }
            Message::ShowMenu(path, is_folder) => {
                if popup_menu(&["エクスプローラーで開く"]) == Some(0)
                    && let Err(error) = show_in_explorer(&path, is_folder)
                {
                    explorer.report_error(format!("{}を開けません: {error}", path.display()));
                }
            }
            Message::ToggleFavorite => explorer.toggle_favorite(),
            Message::RemoveFavorite(path) => explorer.remove_favorite(&path),
            // The view clears the selection again, so the same favorite can be chosen twice.
            Message::FavoriteChosen(Some(row)) => {
                explorer.open_favorite(row);
                // The favorite may be the current folder. Moving there clears the covers, but
                // no folder change loads them again, nor the thumbnails that the grid now needs.
                self.load_previews(context);
            }
            Message::FavoriteChosen(None) => {}
            Message::FavoritesReordered(keys) => {
                explorer.reorder_favorites(keys.into_iter().map(PathBuf::from).collect())
            }
            Message::ToggleViewMode => {
                explorer.toggle_view_mode();
                self.load_thumbnails(context);
            }
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
    /// Only the grid shows them, and each one can take a while, so the list does not ask.
    fn load_thumbnails(&mut self, context: &ComponentContext<Self>) {
        if let Some(task) = self.thumbnails_task.take() {
            task.cancel();
        }
        if self.explorer.view_mode() != ViewMode::Grid {
            return;
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

    fn toolbar(&self, context: &ViewContext<Self>) -> View {
        let explorer = &self.explorer;
        let (star, star_label) = if explorer.is_current_favorite() {
            (Symbol::UnFavorite, "お気に入りから削除")
        } else {
            (Symbol::Favorite, "お気に入りに追加")
        };
        // The button names the view that it switches to.
        let (view_icon, view_label) = match explorer.view_mode() {
            ViewMode::List => (Symbol::ViewAll, "グリッド"),
            ViewMode::Grid => (Symbol::List, "一覧"),
        };
        let labels = crumbs(explorer.current())
            .into_iter()
            .map(|crumb| crumb.label);

        Grid::new()
            .grid_row(0)
            .grid_column_span(2)
            .margin(8.0)
            .column_spacing(8.0)
            .columns([
                GridLength::Auto,
                GridLength::Auto,
                GridLength::Auto,
                GridLength::STAR,
            ])
            .children((
                Button::new()
                    .grid_column(0)
                    .on_click(context.message(Message::ToggleFavorite))
                    .content(icon_label(star, star_label)),
                Button::new()
                    .grid_column(1)
                    .is_enabled(explorer.current().parent().is_some())
                    .on_click(context.message(Message::Up))
                    .content(icon_label(Symbol::Up, "上へ")),
                Button::new()
                    .grid_column(2)
                    .on_click(context.message(Message::ToggleViewMode))
                    .content(icon_label(view_icon, view_label)),
                BreadcrumbBar::new()
                    .grid_column(3)
                    .vertical_alignment(VerticalAlignment::Center)
                    .items_source(labels)
                    .on_item_clicked(context.callback(Message::CrumbClicked)),
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

        ScrollViewer::new().grid_row(1).grid_column(0).content(
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
        let selected = self.explorer.selected();
        let mode = self.explorer.view_mode();
        let items = self
            .explorer
            .entries()
            .iter()
            .enumerate()
            .map(|(row, entry)| {
                let is_selected = selected == Some(row);
                let view = match mode {
                    ViewMode::List => file_row(row, entry, is_selected, context),
                    ViewMode::Grid => {
                        let source = thumbnail_source(&self.explorer, entry);
                        file_tile(row, entry, source, is_selected, context)
                    }
                };
                (entry.name.clone(), view)
            });
        let panel: View = match mode {
            ViewMode::List => StackPanel::new().margin(4.0).keyed_children(items),
            ViewMode::Grid => VariableSizedWrapGrid::new()
                .orientation(Orientation::Horizontal)
                .item_width(TILE_WIDTH)
                .item_height(TILE_HEIGHT)
                .margin(4.0)
                .keyed_children(items),
        };

        ScrollViewer::new()
            .grid_row(1)
            .grid_column(1)
            .content(panel)
    }

    fn status_bar(&self) -> View {
        let explorer = &self.explorer;
        let text = match explorer.error() {
            Some(error) => TextBlock::new()
                .text(error)
                .foreground(ThemeBrush::SystemCritical),
            None => TextBlock::new().text(format!("{}個の項目", explorer.entries().len())),
        };
        text.grid_row(2).grid_column_span(2).margin(8.0).into()
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
                .on_pointer_pressed(folder_menu_handler(path, context))
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
        .on_pointer_pressed(folder_menu_handler(&row.path, context))
        .content(row_view)
}

fn file_row(
    row: usize,
    entry: &Entry,
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
        .padding(Thickness::new(12.0, 8.0, 12.0, 8.0))
        .on_pointer_pressed(press_handler(row, entry, context))
        .content(icon_label(entry_symbol(entry), entry.name.clone()))
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
        .content(
            StackPanel::new().spacing(4.0).children((
                preview(entry, source),
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

/// Shows the thumbnail, or a large icon when there is none.
fn preview(entry: &Entry, source: Option<ThumbnailSource>) -> View {
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
            .width(THUMBNAIL_WIDTH)
            .height(THUMBNAIL_HEIGHT)
            .into(),
        None => Viewbox::new().height(GRID_ICON_HEIGHT).slot(
            ViewboxSlot::Child,
            SymbolIcon::new().symbol(entry_symbol(entry)),
        ),
    };
    let grid = Grid::new().height(THUMBNAIL_HEIGHT);
    if entry.is_folder() && has_thumbnail {
        grid.children((content, folder_badge()))
    } else {
        grid.children((content,))
    }
}

/// A small folder icon on the cover, so that a folder does not look like an image file.
fn folder_badge() -> View {
    Border::new()
        .background(Brush::Theme(ThemeBrush::SolidBackground))
        .corner_radius(4.0)
        .padding(4.0)
        .margin(4.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Bottom)
        .content(SymbolIcon::new().symbol(Symbol::Folder))
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
            sender.send(Message::ShowMenu(path.clone(), is_folder));
        }
    }
}

/// A right press on a favorite or a tree row asks for the context menu of its folder.
fn folder_menu_handler(
    path: &Path,
    context: &ViewContext<ExplorerWindow>,
) -> impl Fn(PointerEventInfo) + use<> {
    let sender = context.sender();
    let path = path.to_path_buf();
    move |pointer: PointerEventInfo| {
        if pointer.is_right_button_pressed {
            sender.send(Message::ShowMenu(path.clone(), true));
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
