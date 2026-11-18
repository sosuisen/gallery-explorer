//! The command line that shows a folder or a file in File Explorer.

use std::path::Path;

/// Returns the arguments of `explorer.exe` that show `path` in File Explorer.
///
/// A folder is opened itself. A file is selected in the folder that holds it.
/// The path is quoted by hand: `/select,` must come before the opening quote, which the
/// quoting of `Command::arg` cannot do.
pub fn explorer_args(path: &Path, is_folder: bool) -> String {
    let quoted = format!("\"{}\"", path.display());
    if is_folder {
        quoted
    } else {
        format!("/select,{quoted}")
    }
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

    // ファイルは、そのファイルを選んだ状態で親フォルダを開く引数になる。
    #[test]
    fn a_file_is_selected_in_its_folder() {
        let args = explorer_args(Path::new(r"D:\my art\一月.png"), false);

        assert_eq!(args, r#"/select,"D:\my art\一月.png""#);
    }
}
