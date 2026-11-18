//! The URI that WinUI loads an image file from.

use std::path::Path;

/// Returns the `file:///` URI of an image file for `Image::source`.
pub fn image_uri(path: &Path) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 日本語の文字は、URIの中にそのまま残る。
    #[test]
    fn japanese_characters_stay_as_they_are() {
        let uri = image_uri(Path::new(r"D:\絵\一月.png"));

        assert_eq!(uri, "file:///D:/絵/一月.png");
    }
}
