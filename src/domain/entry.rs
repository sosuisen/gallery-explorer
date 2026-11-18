use std::path::PathBuf;

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
