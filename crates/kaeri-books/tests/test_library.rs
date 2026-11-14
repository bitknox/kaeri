//! Scans the maintainer's test library when `KAERI_TEST_LIBRARY` names its `books` folder.
//! Skipped otherwise: the library is made-up data kept outside the repository.

use std::path::PathBuf;

use kaeri_books::{Limits, library};

fn root() -> Option<PathBuf> {
    std::env::var_os("KAERI_TEST_LIBRARY").map(PathBuf::from)
}

fn numbers(series: &library::ScannedSeries) -> Vec<String> {
    series
        .files
        .iter()
        .map(|f| match (&f.volume, &f.number) {
            (Some(v), Some(n)) => format!("v{v}c{n}"),
            (None, Some(n)) => format!("c{n}"),
            (Some(v), None) => format!("v{v}"),
            (None, None) => f.label.clone(),
        })
        .collect()
}

#[test]
fn the_test_library_reads_as_the_readme_says() {
    let Some(root) = root() else {
        eprintln!("KAERI_TEST_LIBRARY is not set; skipped");
        return;
    };
    let scan =
        library::scan(&root, &library::Nothing, &Limits::default(), &mut |_| {}).unwrap();
    let find = |name: &str| {
        scan.series.iter().find(|s| s.name == name).unwrap_or_else(|| {
            panic!("no {name} in {:?}", scan.series.iter().map(|s| &s.name).collect::<Vec<_>>())
        })
    };

    let moonlit = find("Moonlit Harbor");
    assert_eq!(numbers(moonlit), ["v1", "v2", "v3", "v4"]);
    assert_eq!(moonlit.right_to_left, Some(true));
    assert_eq!(moonlit.files[3].format, "cb7");

    let tin = find("Tin Lantern Club");
    assert_eq!(numbers(tin), ["c1", "c2", "c3", "c9", "c10", "c10.5", "c11"]);
    assert!(!tin.restarts_per_volume);

    let kites = find("Paper Kite Rangers");
    assert!(kites.restarts_per_volume);
    assert_eq!(numbers(kites), ["v1c1", "v1c2", "v1c3", "v2c1", "v2c2", "v2c3"]);

    assert_eq!(find("Tower of Ash").files.len(), 2);
    assert_eq!(find("Glass Orchard").files[0].format, "pdf");
    assert_eq!(find("Glass Orchard").files[0].pages, Some(6));

    let novels = scan.series.iter().filter(|s| s.novel).count();
    assert_eq!(novels, 2, "the two EPUBs are novels");
    // A book directly in the root is a series of its own; its ComicInfo names it.
    let loose = scan.series.iter().find(|s| s.key == "Loose Single Volume").unwrap();
    assert_eq!((loose.name.as_str(), loose.files.len()), ("Double Page Spread", 1));
    // The zero byte file and the text file are set aside; the corrupt image book still opens.
    assert!(scan.ignored >= 2);
    let edge = find("Edge Cases");
    assert!(
        edge.files.iter().any(|f| f.path.ends_with("Corrupt Image.cbz") && f.broken.is_none())
    );
    assert!(
        edge.files.iter().any(|f| f.path.ends_with("Actually A Zip.cbr") && f.broken.is_none())
    );
}
