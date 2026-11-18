//! The URI that WinUI loads an image file from.

use std::path::Path;

/// Returns the `file:///` URI of an image file for `Image::source`.
///
/// Characters outside ASCII stay as they are. WinUI cannot open a file whose name is
/// percent-encoded as UTF-8, which is what `Image::source_file` does.
pub fn image_uri(path: &Path) -> String {
    let mut uri = String::from("file:///");
    for c in path.to_string_lossy().chars() {
        match c {
            '\\' => uri.push('/'),
            c if !c.is_ascii() || c.is_ascii_alphanumeric() || "-._~/:".contains(c) => uri.push(c),
            c => uri.push_str(&format!("%{:02X}", c as u32)),
        }
    }
    uri
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

    // 空白と`#`と`%`は、URIの中でエンコードされる。
    #[test]
    fn spaces_hashes_and_percent_signs_are_encoded() {
        let uri = image_uri(Path::new(r"D:\my art\#1 100%.png"));

        assert_eq!(uri, "file:///D:/my%20art/%231%20100%25.png");
    }
}
