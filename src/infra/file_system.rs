use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::domain::{Entry, EntryKind, sort_entries};

/// Reads the items of a folder, sorted for display.
pub fn read_folder(path: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(path)? {
        let item = item?;
        let path = item.path();
        // A link reports the type of the link itself, so its target is checked through the path.
        let kind = if path.is_dir() {
            EntryKind::Folder
        } else {
            EntryKind::File
        };
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            path,
            kind,
        });
    }
    sort_entries(&mut entries);
    Ok(entries)
}

/// Reads the folders directly inside a folder, sorted for display.
pub fn child_folders(path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = read_folder(path)?;
    Ok(entries
        .into_iter()
        .filter(Entry::is_folder)
        .map(|entry| entry.path)
        .collect())
}

/// Lists the drive roots that exist now, from `A:\` to `Z:\`.
pub fn drives() -> Vec<PathBuf> {
    ('A'..='Z')
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|root| root.exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // フォルダの中身が、フォルダを先にして名前順に読み込まれる。
    #[test]
    fn folder_items_are_read_with_folders_first_in_name_order() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("b.txt"), "").unwrap();
        fs::write(folder.path().join("a.txt"), "").unwrap();
        fs::create_dir(folder.path().join("sub")).unwrap();

        let entries = read_folder(folder.path()).unwrap();

        let items: Vec<(&str, EntryKind)> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.kind))
            .collect();
        assert_eq!(
            items,
            [
                ("sub", EntryKind::Folder),
                ("a.txt", EntryKind::File),
                ("b.txt", EntryKind::File),
            ]
        );
        assert_eq!(entries[0].path, folder.path().join("sub"));
    }

    // 存在しないフォルダの読み込みは、エラーになる。
    #[test]
    fn reading_a_missing_folder_is_an_error() {
        let folder = tempfile::tempdir().unwrap();

        assert!(read_folder(&folder.path().join("missing")).is_err());
    }

    // ドライブの一覧には、一時フォルダのあるドライブが含まれる。
    #[test]
    fn drives_include_the_drive_of_the_temporary_folder() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().ancestors().last().unwrap();

        assert!(drives().iter().any(|drive| drive == root));
    }

    // 子フォルダの一覧には、ファイルが含まれない。
    #[test]
    fn child_folders_do_not_include_files() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("a.txt"), "").unwrap();
        fs::create_dir(folder.path().join("sub")).unwrap();

        let children = child_folders(folder.path()).unwrap();

        assert_eq!(children, [folder.path().join("sub")]);
    }
}
