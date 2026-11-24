use std::io;
use std::path::Path;
use std::process::Command;

use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::Shell::{
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_WANTNUKEWARNING, FOFX_RECYCLEONDELETE, FileOperation,
    IFileOperation, ILCreateFromPathW, ILFree, IShellItem, SHCreateItemFromParsingName,
    SHOpenFolderAndSelectItems,
};
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

/// Moves a file or a folder to the Recycle Bin, as File Explorer does.
///
/// It asks nothing. Only an item that cannot go to the Recycle Bin, such as one on a network
/// share, makes Windows ask whether to delete it for good. It is never deleted silently.
/// It runs on the UI thread, where COM is ready, and shows the progress for a large folder.
pub fn move_to_recycle_bin(path: &Path) -> io::Result<()> {
    unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(io::Error::other)?;
        operation
            .SetOperationFlags(
                FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING | FOFX_RECYCLEONDELETE,
            )
            .map_err(io::Error::other)?;
        // Without an owner, the warning and the progress could open behind the app.
        let window = GetActiveWindow();
        if !window.is_invalid() {
            operation.SetOwnerWindow(window).map_err(io::Error::other)?;
        }
        let item: IShellItem =
            SHCreateItemFromParsingName(&HSTRING::from(path), None).map_err(io::Error::other)?;
        operation
            .DeleteItem(&item, None)
            .map_err(io::Error::other)?;
        operation.PerformOperations().map_err(io::Error::other)?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(io::Error::other)?
            .as_bool()
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "削除は取り消されました",
            ));
        }
    }
    Ok(())
}
