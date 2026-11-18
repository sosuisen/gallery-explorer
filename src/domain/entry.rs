use std::path::PathBuf;
use std::time::SystemTime;

/// File extensions of the images that WinUI can show. They are in lowercase.
const IMAGE_EXTENSIONS: [&str; 10] = [
    "bmp", "gif", "ico", "jpeg", "jpg", "png", "tif", "tiff", "webp", "jxr",
];

/// Whether an entry is a folder or a file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Folder,
    File,
}

/// One item in a folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    pub modified: SystemTime,
}

impl Entry {
    pub fn is_folder(&self) -> bool {
        self.kind == EntryKind::Folder
    }

    /// Returns whether the entry is an image file that can be shown as a thumbnail.
    /// The extension decides it, and letter case does not matter.
    pub fn is_image(&self) -> bool {
        self.kind == EntryKind::File
            && self
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                })
    }
}

/// The order of the items in a folder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortOrder {
    #[default]
    NameAscending,
    NameDescending,
    ModifiedAscending,
    ModifiedDescending,
}

/// Sorts folders before files. Inside each group, the order ignores letter case.
pub fn sort_entries(entries: &mut [Entry], order: SortOrder) {
    entries.sort_by_cached_key(|entry| (!entry.is_folder(), entry.name.to_lowercase()));
    if order == SortOrder::NameDescending {
        let files = entries.partition_point(Entry::is_folder);
        entries[..files].reverse();
        entries[files..].reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        modified_entry(name, kind, 0)
    }

    /// An entry that was modified the given number of seconds after the epoch.
    fn modified_entry(name: &str, kind: EntryKind, seconds: u64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        }
    }

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    // 画像の拡張子のファイルは、大文字と小文字を問わず画像である。
    #[test]
    fn file_with_an_image_extension_is_an_image_in_any_letter_case() {
        assert!(entry("a.jpg", EntryKind::File).is_image());
        assert!(entry("a.PNG", EntryKind::File).is_image());
    }

    // 画像でない拡張子のファイルと、拡張子のないファイルは、画像でない。
    #[test]
    fn file_without_an_image_extension_is_not_an_image() {
        assert!(!entry("a.txt", EntryKind::File).is_image());
        assert!(!entry("jpg", EntryKind::File).is_image());
    }

    // 画像の拡張子の名前を持つフォルダは、画像でない。
    #[test]
    fn folder_with_an_image_extension_is_not_an_image() {
        assert!(!entry("photos.jpg", EntryKind::Folder).is_image());
    }

    // フォルダはファイルより前に並ぶ。
    #[test]
    fn folders_come_before_files() {
        let mut entries = vec![
            entry("a.txt", EntryKind::File),
            entry("z", EntryKind::Folder),
        ];

        sort_entries(&mut entries, SortOrder::NameAscending);

        assert_eq!(names(&entries), ["z", "a.txt"]);
    }

    // 名前の順序は大文字と小文字を区別しない。
    #[test]
    fn name_order_ignores_letter_case() {
        let mut entries = vec![
            entry("b.txt", EntryKind::File),
            entry("C.txt", EntryKind::File),
            entry("A.txt", EntryKind::File),
        ];

        sort_entries(&mut entries, SortOrder::NameAscending);

        assert_eq!(names(&entries), ["A.txt", "b.txt", "C.txt"]);
    }

    // 名前の降順では、名前の逆順に並ぶ。フォルダはファイルより前のままである。
    #[test]
    fn name_descending_reverses_names_and_keeps_folders_first() {
        let mut entries = vec![
            entry("a.txt", EntryKind::File),
            entry("B.txt", EntryKind::File),
            entry("x", EntryKind::Folder),
            entry("y", EntryKind::Folder),
        ];

        sort_entries(&mut entries, SortOrder::NameDescending);

        assert_eq!(names(&entries), ["y", "x", "B.txt", "a.txt"]);
    }

    // 更新日の昇順では、古いものから並ぶ。フォルダはファイルより前のままである。
    #[test]
    fn modified_ascending_puts_older_entries_first() {
        let mut entries = vec![
            modified_entry("a.txt", EntryKind::File, 30),
            modified_entry("b.txt", EntryKind::File, 10),
            modified_entry("x", EntryKind::Folder, 20),
            modified_entry("y", EntryKind::Folder, 5),
        ];

        sort_entries(&mut entries, SortOrder::ModifiedAscending);

        assert_eq!(names(&entries), ["y", "x", "b.txt", "a.txt"]);
    }
}
