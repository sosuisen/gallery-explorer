use std::io;
use std::path::Path;
use std::process::Command;

/// Opens a file with the app that Windows assigns to its type.
pub fn open_with_default_app(path: &Path) -> io::Result<()> {
    Command::new("explorer.exe").arg(path).spawn().map(drop)
}
