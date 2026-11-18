use std::path::{Path, PathBuf};

/// One step of the path from the drive to the current folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Crumb {
    pub label: String,
    pub path: PathBuf,
}

/// Returns the name to show for a folder or a file.
///
/// A drive root has no file name. Its label is the path without the last separator: `C:`.
pub fn display_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path
            .to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_string(),
    }
}

/// Splits a path into crumbs, from the drive to the folder itself.
pub fn crumbs(path: &Path) -> Vec<Crumb> {
    let mut crumbs: Vec<Crumb> = path
        .ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .map(|ancestor| Crumb {
            label: display_name(ancestor),
            path: ancestor.to_path_buf(),
        })
        .collect();
    crumbs.reverse();
    crumbs
}

/// Finds the folder for a clicked crumb label.
///
/// The breadcrumb control reports only the label. When two crumbs share a label, the deepest
/// parent wins. The current folder is the last choice, because a click on it changes nothing.
pub fn resolve_crumb(path: &Path, label: &str) -> Option<PathBuf> {
    let crumbs = crumbs(path);
    let (current, parents) = crumbs.split_last()?;
    parents
        .iter()
        .rev()
        .chain([current])
        .find(|crumb| crumb.label == label)
        .map(|crumb| crumb.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ドライブのルートは、末尾の区切りを除いた名前で表示される。
    #[test]
    fn drive_root_is_shown_without_the_last_separator() {
        assert_eq!(display_name(Path::new(r"C:\")), "C:");
    }

    // フォルダは、その名前で表示される。
    #[test]
    fn folder_is_shown_by_its_name() {
        assert_eq!(display_name(Path::new(r"C:\Users\me")), "me");
    }

    // パスは、ドライブから現在のフォルダまでのパンくずに分かれる。
    #[test]
    fn path_is_split_into_crumbs_from_the_drive_to_the_folder() {
        let crumbs = crumbs(Path::new(r"C:\Users\me"));

        let labels: Vec<&str> = crumbs.iter().map(|crumb| crumb.label.as_str()).collect();
        let paths: Vec<&Path> = crumbs.iter().map(|crumb| crumb.path.as_path()).collect();
        assert_eq!(labels, ["C:", "Users", "me"]);
        assert_eq!(
            paths,
            [
                Path::new(r"C:\"),
                Path::new(r"C:\Users"),
                Path::new(r"C:\Users\me")
            ]
        );
    }

    // パンくずのラベルから、そのフォルダのパスが求まる。
    #[test]
    fn crumb_label_resolves_to_its_folder() {
        let path = resolve_crumb(Path::new(r"C:\Users\me"), "Users");

        assert_eq!(path, Some(PathBuf::from(r"C:\Users")));
    }

    // 同じラベルが現在のフォルダと親にあるときは、親が選ばれる。
    #[test]
    fn parent_wins_when_the_current_folder_has_the_same_label() {
        let path = resolve_crumb(Path::new(r"C:\data\work\data"), "data");

        assert_eq!(path, Some(PathBuf::from(r"C:\data")));
    }

    // パンくずにないラベルは、どのフォルダにもならない。
    #[test]
    fn unknown_label_resolves_to_no_folder() {
        assert_eq!(resolve_crumb(Path::new(r"C:\Users"), "other"), None);
    }
}
