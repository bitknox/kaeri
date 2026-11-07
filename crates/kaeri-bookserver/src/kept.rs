//! Where a downloaded book is kept while it is read.

use std::path::{Path, PathBuf};

/// A file name for a book fetched from `source`, under `dir`. The engine removes it when it is
/// done with it, since the content says it is temporary.
pub fn keep_path(dir: &Path, source: &str, book: &str, file: &str) -> PathBuf {
    let safe = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }
            })
            .take(80)
            .collect::<String>()
            .trim_start_matches('.')
            .to_owned()
    };
    dir.join(safe(source)).join(safe(book)).join(safe(file))
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
}
