//! The command line that shows a folder or a file in File Explorer.

use std::path::Path;

/// Returns the arguments of `explorer.exe` that show `path` in File Explorer.
pub fn explorer_args(path: &Path, is_folder: bool) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // フォルダは、そのフォルダを開く引数になる。
    #[test]
    fn a_folder_is_opened_itself() {
        let args = explorer_args(Path::new(r"D:\my art"), true);

        assert_eq!(args, r#""D:\my art""#);
    }
}
