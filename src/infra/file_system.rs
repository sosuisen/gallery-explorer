use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::domain::{Entry, EntryKind, SortOrder, sort_entries};

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
            modified: SystemTime::UNIX_EPOCH,
        });
    }
    sort_entries(&mut entries, SortOrder::NameAscending);
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

/// Returns the first image file directly inside a folder, in display order.
fn first_image(folder: &Path) -> Option<PathBuf> {
    read_folder(folder)
        .ok()?
        .into_iter()
        .find(Entry::is_image)
        .map(|entry| entry.path)
}

/// Finds the first image of each folder. Folders without an image are left out.
///
/// It stops early, with the covers found so far, when `cancelled` returns `true`.
pub fn folder_covers(
    folders: impl IntoIterator<Item = PathBuf>,
    cancelled: impl Fn() -> bool,
) -> Vec<(PathBuf, PathBuf)> {
    let mut covers = Vec::new();
    for folder in folders {
        if cancelled() {
            break;
        }
        if let Some(image) = first_image(&folder) {
            covers.push((folder, image));
        }
    }
    covers
}

/// Makes a thumbnail for each file with `thumbnail_of`. Files without one are left out.
///
/// It stops early, with the thumbnails made so far, when `cancelled` returns `true`.
pub fn collect_thumbnails(
    files: impl IntoIterator<Item = PathBuf>,
    cancelled: impl Fn() -> bool,
    thumbnail_of: impl Fn(&Path) -> Option<Vec<u8>>,
) -> Vec<(PathBuf, Arc<[u8]>)> {
    let mut thumbnails = Vec::new();
    for file in files {
        if cancelled() {
            break;
        }
        if let Some(bytes) = thumbnail_of(&file) {
            thumbnails.push((file, Arc::from(bytes)));
        }
    }
    thumbnails
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

    // フォルダを読むと、項目の更新日がわかる。
    #[test]
    fn folder_items_are_read_with_their_modified_time() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("a.txt");
        fs::write(&file, "").unwrap();
        let modified = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified)
            .unwrap();

        let entries = read_folder(folder.path()).unwrap();

        assert_eq!(entries[0].modified, modified);
    }

    // 存在しないフォルダの読み込みは、エラーになる。
    #[test]
    fn reading_a_missing_folder_is_an_error() {
        let folder = tempfile::tempdir().unwrap();

        assert!(read_folder(&folder.path().join("missing")).is_err());
    }

    // フォルダの1枚目の画像は、名前順で最初の画像ファイルである。
    #[test]
    fn first_image_is_the_first_image_file_in_name_order() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("a.txt"), "").unwrap();
        fs::write(folder.path().join("C.png"), "").unwrap();
        fs::write(folder.path().join("b.jpg"), "").unwrap();

        assert_eq!(
            first_image(folder.path()),
            Some(folder.path().join("b.jpg"))
        );
    }

    // 子フォルダの中の画像は、1枚目の画像にならない。
    #[test]
    fn first_image_ignores_images_in_sub_folders() {
        let folder = tempfile::tempdir().unwrap();
        fs::create_dir(folder.path().join("sub")).unwrap();
        fs::write(folder.path().join("sub").join("a.jpg"), "").unwrap();
        fs::write(folder.path().join("note.txt"), "").unwrap();

        assert_eq!(first_image(folder.path()), None);
    }

    // 読めないフォルダには、1枚目の画像がない。
    #[test]
    fn unreadable_folder_has_no_first_image() {
        let folder = tempfile::tempdir().unwrap();

        assert_eq!(first_image(&folder.path().join("missing")), None);
    }

    // 画像のあるフォルダだけが、その1枚目の画像と組になる。
    #[test]
    fn covers_pair_only_the_folders_with_an_image() {
        let folder = tempfile::tempdir().unwrap();
        let photos = folder.path().join("photos");
        let empty = folder.path().join("empty");
        fs::create_dir(&photos).unwrap();
        fs::create_dir(&empty).unwrap();
        fs::write(photos.join("a.jpg"), "").unwrap();

        let covers = folder_covers([empty, photos.clone()], || false);

        assert_eq!(covers, [(photos.clone(), photos.join("a.jpg"))]);
    }

    // 取り消されると、残りのフォルダは調べない。
    #[test]
    fn covers_stop_when_cancelled() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("a.jpg"), "").unwrap();

        assert!(folder_covers([folder.path().to_path_buf()], || true).is_empty());
    }

    // サムネイルを作れたファイルだけが、そのサムネイルと組になる。
    #[test]
    fn thumbnails_pair_only_the_files_that_have_one() {
        let clip = PathBuf::from("a.clip");
        let text = PathBuf::from("b.txt");

        let thumbnails = collect_thumbnails(
            [clip.clone(), text],
            || false,
            |file| (file.extension()? == "clip").then(|| vec![1, 2, 3]),
        );

        assert_eq!(thumbnails, [(clip, Arc::from(vec![1, 2, 3]))]);
    }

    // 取り消されると、残りのファイルのサムネイルは作らない。
    #[test]
    fn thumbnails_stop_when_cancelled() {
        let thumbnails = collect_thumbnails([PathBuf::from("a.clip")], || true, |_| Some(vec![1]));

        assert!(thumbnails.is_empty());
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
