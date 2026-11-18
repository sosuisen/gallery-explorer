//! The explorer window: it turns the explorer state into WinUI controls.

use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

use windows_reactor::*;

use crate::domain::{Entry, TreeRow, crumbs, display_name};
use crate::explorer::Explorer;
use crate::infra::{FavoritesStore, drives, open_with_default_app};

const SIDE_WIDTH: f64 = 300.0;
const TREE_INDENT: f64 = 16.0;
// A gray with low alpha shows on both the light and the dark theme.
const SELECTED_BACKGROUND: Color = Color::argb(0x28, 0x80, 0x80, 0x80);

#[derive(Clone)]
pub enum Message {
    Navigate(PathBuf),
    Up,
    CrumbClicked(String),
    ToggleTree(PathBuf),
    OpenTreeFolder(PathBuf),
    RowPressed(usize, Instant),
    ToggleFavorite,
    RemoveFavorite(PathBuf),
}

pub struct ExplorerWindow {
    explorer: Explorer,
}

impl Component for ExplorerWindow {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        let roots = drives();
        let start = env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| roots.first().cloned())
            .unwrap_or_default();
        Self {
            explorer: Explorer::open(roots, &start, FavoritesStore::in_app_data()),
        }
    }

    fn update(&mut self, message: Message, _context: &ComponentContext<Self>) {
        let explorer = &mut self.explorer;
        match message {
            Message::Navigate(path) => explorer.navigate(&path),
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
            Message::ToggleFavorite => explorer.toggle_favorite(),
            Message::RemoveFavorite(path) => explorer.remove_favorite(&path),
        }
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
    fn toolbar(&self, context: &ViewContext<Self>) -> View {
        let explorer = &self.explorer;
        let (star, star_label) = if explorer.is_current_favorite() {
            (Symbol::UnFavorite, "お気に入りから削除")
        } else {
            (Symbol::Favorite, "お気に入りに追加")
        };
        let labels = crumbs(explorer.current())
            .into_iter()
            .map(|crumb| crumb.label);

        Grid::new()
            .grid_row(0)
            .grid_column_span(2)
            .margin(8.0)
            .column_spacing(8.0)
            .columns([GridLength::Auto, GridLength::Auto, GridLength::STAR])
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
                BreadcrumbBar::new()
                    .grid_column(2)
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
            .map(|path| (key(path), favorite_row(path, context)));
        let tree = self
            .explorer
            .tree_rows()
            .into_iter()
            .map(|row| (key(&row.path), tree_row(&row, context)));

        ScrollViewer::new().grid_row(1).grid_column(0).content(
            StackPanel::new().margin(8.0).spacing(4.0).children((
                heading("お気に入り"),
                StackPanel::new().keyed_children(favorites),
                heading("フォルダ"),
                StackPanel::new().keyed_children(tree),
            )),
        )
    }

    // A ListView animates its items when they change, and windows-reactor cannot turn that off.
    // A StackPanel has no animation, so a folder change shows the new items at once.
    // An ItemsRepeater has no animation either, but windows-reactor puts each of its rows in a
    // left-aligned container, so a row cannot fill the width.
    fn file_list(&self, context: &ViewContext<Self>) -> View {
        let selected = self.explorer.selected();
        let rows = self
            .explorer
            .entries()
            .iter()
            .enumerate()
            .map(|(row, entry)| {
                let view = file_row(row, entry, selected == Some(row), context);
                (entry.name.clone(), view)
            });

        ScrollViewer::new()
            .grid_row(1)
            .grid_column(1)
            .content(StackPanel::new().margin(4.0).keyed_children(rows))
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
            Button::new()
                .grid_column(0)
                .style(ButtonStyle::Subtle)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .horizontal_content_alignment(HorizontalAlignment::Left)
                .on_click(context.message(Message::Navigate(path.to_path_buf())))
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

    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .margin(Thickness::new(indent, 0.0, 0.0, 0.0))
        .children((
            Button::new()
                .style(ButtonStyle::Subtle)
                .on_click(context.message(Message::ToggleTree(row.path.clone())))
                .content(TextBlock::new().text(arrow).font_size(10.0)),
            Button::new()
                .style(ButtonStyle::Subtle)
                .on_click(context.message(Message::OpenTreeFolder(row.path.clone())))
                .content(icon_label(Symbol::Folder, display_name(&row.path))),
        ))
}

fn file_row(
    row: usize,
    entry: &Entry,
    selected: bool,
    context: &ViewContext<ExplorerWindow>,
) -> View {
    let symbol = if entry.is_folder() {
        Symbol::Folder
    } else {
        Symbol::Document
    };
    // The selected row gets a gray background and an accent bar on the left. Every row keeps the
    // bar's width, so the text does not move when the selection changes.
    let (background, bar) = if selected {
        (SELECTED_BACKGROUND, Brush::Theme(ThemeBrush::Accent))
    } else {
        // A border without a background receives no pointer events.
        (Color::transparent(), Brush::Solid(Color::transparent()))
    };
    // The rows report no double click, so each row reports every left press with its time.
    let sender = context.sender();
    let pressed = move |pointer: PointerEventInfo| {
        if pointer.is_left_button_pressed {
            sender.send(Message::RowPressed(row, Instant::now()));
        }
    };

    Border::new()
        .background(background)
        .border_brush(bar)
        .border_thickness(Thickness::new(3.0, 0.0, 0.0, 0.0))
        .corner_radius(4.0)
        .padding(Thickness::new(12.0, 8.0, 12.0, 8.0))
        .on_pointer_pressed(pressed)
        .content(icon_label(symbol, entry.name.clone()))
}
