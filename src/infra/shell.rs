use std::io;
use std::path::Path;
use std::process::Command;

use windows::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
use windows::core::HSTRING;

/// Opens a file with the app that Windows assigns to its type.
pub fn open_with_default_app(path: &Path) -> io::Result<()> {
    Command::new("explorer.exe").arg(path).spawn().map(drop)
}

/// Shows a folder, or a file in the folder that holds it, in File Explorer.
pub fn show_in_explorer(path: &Path, is_folder: bool) -> io::Result<()> {
    if is_folder {
        open_with_default_app(path)
    } else {
        select_in_explorer(path)
    }
}

/// Opens the folder that holds the file, with the file selected.
///
/// Not `explorer /select,`: on Windows 11 it opened the folder but did not select the file.
fn select_in_explorer(path: &Path) -> io::Result<()> {
    unsafe {
        let item = ILCreateFromPathW(&HSTRING::from(path));
        if item.is_null() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such file"));
        }
        let result = SHOpenFolderAndSelectItems(item, None, 0);
        ILFree(Some(item));
        result.map_err(io::Error::other)
    }
}
