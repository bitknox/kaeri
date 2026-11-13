//! Where a downloaded book is kept while it is read.

use std::path::{Path, PathBuf};

/// A file name for a book fetched from `source`, under `dir`. The engine removes it when it is
/// done with it, since the content says it is temporary. Each part is short enough for any file
/// system, and two books never share a path ([`kaeri_core::file_name::of_id`]); the file keeps
/// its extension.
pub fn keep_path(dir: &Path, source: &str, book: &str, file: &str) -> PathBuf {
    let name = kaeri_core::file_name::of_id;
    let file = match file.rsplit_once('.') {
        Some((stem, ext))
            if !ext.is_empty()
                && ext.len() <= 8
                && ext.bytes().all(|b| b.is_ascii_alphanumeric()) =>
        {
            format!("{}.{}", name(stem), ext.to_ascii_lowercase())
        }
        _ => name(file),
    };
    dir.join(name(source)).join(name(book)).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_from_a_server_cannot_leave_its_folder() {
        let path = keep_path(Path::new("/cache"), "srv", "../../etc", "a/b.cbz");
        assert!(path.starts_with("/cache/srv"));
        assert!(path.components().all(|c| c.as_os_str() != ".."), "{path:?}");
    }

    #[test]
    fn books_with_long_or_alike_ids_keep_files_of_their_own() {
        let long = "https://library.example/opds/catalog/books/by-series/the-long-road/volume";
        let a = keep_path(Path::new("/cache"), "srv", &format!("{long}-1"), "book.epub");
        let b = keep_path(Path::new("/cache"), "srv", &format!("{long}-2"), "book.epub");
        assert_ne!(a, b);
        let (c, d) = (
            keep_path(Path::new("/cache"), "srv", "a b", "book.cbz"),
            keep_path(Path::new("/cache"), "srv", "a_b", "book.cbz"),
        );
        assert_ne!(c, d);
        for path in [&a, &b, &c, &d] {
            for part in path.strip_prefix("/cache").unwrap().components() {
                assert!(part.as_os_str().len() <= 80, "{path:?}");
            }
            assert!(path.extension().is_some(), "{path:?}");
        }
        let kanji: String = std::iter::repeat_n('灯', 90).collect();
        let e = keep_path(Path::new("/cache"), "srv", &kanji, &format!("{kanji}.pdf"));
        assert!(e.file_name().unwrap().len() <= 80, "{e:?}");
        assert_eq!(e.extension().unwrap(), "pdf");
        let reserved = keep_path(Path::new("/cache"), "srv", "con", "nul.cbz");
        assert!(!kaeri_core::file_name::reserved_on_windows(
            reserved.file_name().unwrap().to_str().unwrap()
        ));
    }
}
