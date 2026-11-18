use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::domain::Favorites;

/// Keeps the favorites in a JSON file.
#[derive(Debug)]
pub struct FavoritesStore {
    file: PathBuf,
}

impl FavoritesStore {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// Uses `%APPDATA%\GalleryExplorer\favorites.json`.
    pub fn in_app_data() -> Self {
        let app_data = env::var_os("APPDATA").map_or_else(env::temp_dir, PathBuf::from);
        Self::new(app_data.join("GalleryExplorer").join("favorites.json"))
    }

    /// Loads the favorites. A file that does not exist yet means no favorites.
    pub fn load(&self) -> io::Result<Favorites> {
        match fs::read_to_string(&self.file) {
            Ok(json) => Ok(serde_json::from_str(&json)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Favorites::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, favorites: &Favorites) -> io::Result<()> {
        if let Some(folder) = self.file.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(&self.file, serde_json::to_string_pretty(favorites)?)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    // 保存したお気に入りは、読み込むと元に戻る。
    #[test]
    fn saved_favorites_come_back_when_loaded() {
        let folder = tempfile::tempdir().unwrap();
        let store = FavoritesStore::new(folder.path().join("sub").join("favorites.json"));
        let mut favorites = Favorites::default();
        favorites.toggle(Path::new(r"C:\a"));
        favorites.toggle(Path::new(r"D:\写真"));

        store.save(&favorites).unwrap();

        assert_eq!(store.load().unwrap(), favorites);
    }

    // ファイルがまだないときは、お気に入りは空である。
    #[test]
    fn favorites_are_empty_when_the_file_does_not_exist_yet() {
        let folder = tempfile::tempdir().unwrap();
        let store = FavoritesStore::new(folder.path().join("favorites.json"));

        assert_eq!(store.load().unwrap(), Favorites::default());
    }

    // 壊れたファイルの読み込みは、エラーになる。
    #[test]
    fn loading_a_broken_file_is_an_error() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("favorites.json");
        fs::write(&file, "not json").unwrap();

        assert!(FavoritesStore::new(file).load().is_err());
    }
}
