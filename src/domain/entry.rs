use std::path::PathBuf;

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

/// Sorts folders before files. Inside each group, the order ignores letter case.
pub fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by_cached_key(|entry| (!entry.is_folder(), entry.name.to_lowercase()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from(name),
            kind,
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

        sort_entries(&mut entries);

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

        sort_entries(&mut entries);

        assert_eq!(names(&entries), ["A.txt", "b.txt", "C.txt"]);
    }
}
