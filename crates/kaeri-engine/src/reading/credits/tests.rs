use super::*;
use image::{GrayImage, ImageEncoder, Luma, codecs::jpeg::JpegEncoder};

const WORK: &str = "a:900401";

/// A page of generated blocks: `seed` 0 is the group's credit page, any other a story page.
fn page(seed: u32) -> Vec<u8> {
    let (w, h) = (600u32, 860u32);
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(977);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let mut img = GrayImage::from_pixel(w, h, Luma([248]));
    for _ in 0..7 {
        let (x0, y0) = (next() % (w - 120), next() % (h - 160));
        let (x1, y1) = (x0 + 60 + next() % 120, y0 + 80 + next() % 160);
        let tone = (next() % 200) as u8;
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                img.put_pixel(x, y, Luma([tone]));
            }
        }
    }
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 80)
        .write_image(&img, w, h, image::ExtendedColorType::L8)
        .unwrap();
    out
}

/// A chapter's pages in a folder: story pages, with the credit page where `credit_at` says.
fn chapter(
    dir: &Path,
    name: &str,
    pages: u32,
    credit_at: Option<u32>,
    story: u32,
) -> Vec<std::path::PathBuf> {
    let dir = dir.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    (0..pages)
        .map(|i| {
            let bytes = if Some(i) == credit_at { page(0) } else { page(story * 100 + i + 1) };
            let path = dir.join(format!("{i:05}.jpg"));
            std::fs::write(&path, bytes).unwrap();
            path
        })
        .collect()
}

fn check(store: &Store, chapter: &str) -> CreditCheck {
    CreditCheck { store: store.clone(), work: WORK.into(), chapter: chapter.into(), now: 1 }
}

fn find(store: &Store, name: &str, files: &[std::path::PathBuf]) -> Vec<u32> {
    let files: Vec<Option<&Path>> = files.iter().map(|p| Some(p.as_path())).collect();
    check(store, name).find(&files).into_iter().map(|p| p.page).collect()
}

#[test]
fn edges_are_each_parts_first_and_last_two_pages() {
    assert_eq!(edges(&[(0, 10)]), vec![(0, false), (1, false), (8, true), (9, true)]);
    assert_eq!(edges(&[(0, 3)]), vec![(0, false), (1, false), (2, true)]);
    assert_eq!(
        edges(&[(0, 5), (5, 4)]),
        vec![
            (0, false),
            (1, false),
            (3, true),
            (4, true),
            (5, false),
            (6, false),
            (7, true),
            (8, true)
        ]
    );
}

#[test]
fn a_page_at_the_end_of_two_chapters_stays_and_of_three_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let one = chapter(dir.path(), "1", 12, Some(11), 1);
    let two = chapter(dir.path(), "2", 12, Some(11), 2);
    assert!(find(&store, "1", &one).is_empty(), "one chapter is no pattern");
    assert!(find(&store, "2", &two).is_empty(), "two chapters are not enough");
    let three = chapter(dir.path(), "3", 14, Some(13), 3);
    assert_eq!(find(&store, "3", &three), vec![13], "three are");
    // Learnt now: the first two chapters skip it too when opened again.
    assert_eq!(find(&store, "1", &one), vec![11]);
    // Story pages at the edges never match one another.
    let four = chapter(dir.path(), "4", 12, None, 4);
    assert!(find(&store, "4", &four).is_empty());
}

#[test]
fn the_credit_page_in_the_middle_of_a_chapter_is_never_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    for (n, story) in [("1", 1), ("2", 2), ("3", 3)] {
        let files = chapter(dir.path(), n, 12, Some(0), story);
        find(&store, n, &files);
    }
    // The same page at the start of three chapters is a credit page...
    let four = chapter(dir.path(), "4", 12, Some(0), 4);
    assert_eq!(find(&store, "4", &four), vec![0]);
    // ...and in the middle of one it is a page of the chapter: middles are never fingerprinted.
    let five = chapter(dir.path(), "5", 12, Some(6), 5);
    assert!(find(&store, "5", &five).is_empty());
    assert!(
        store.page_prints(WORK).unwrap().iter().all(|p| !(p.chapter == "5" && p.page == 6))
    );
}

#[test]
fn a_page_marked_not_a_credit_page_is_never_skipped_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    let mut last = Vec::new();
    for (n, story) in [("1", 1), ("2", 2), ("3", 3)] {
        last = chapter(dir.path(), n, 10, Some(9), story);
        find(&store, n, &last);
    }
    let files: Vec<Option<&Path>> = last.iter().map(|p| Some(p.as_path())).collect();
    let found = check(&store, "3").find(&files);
    assert_eq!(found.len(), 1);
    store.keep_credit_page(WORK, found[0].print, 2).unwrap();
    assert!(find(&store, "3", &last).is_empty());
    // A fourth chapter with the page keeps it too.
    let four = chapter(dir.path(), "4", 10, Some(9), 4);
    assert!(find(&store, "4", &four).is_empty());
    // Another series is not told.
    assert!(store.kept_credit_pages("a:900402").unwrap().is_empty());
}

#[test]
fn a_chapter_of_credit_pages_alone_is_shown_whole() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().unwrap();
    for (n, story) in [("1", 1), ("2", 2), ("3", 3)] {
        let files = chapter(dir.path(), n, 10, Some(0), story);
        find(&store, n, &files);
    }
    let only = chapter(dir.path(), "4", 1, Some(0), 4);
    assert!(find(&store, "4", &only).is_empty());
}

#[tokio::test]
async fn the_recorder_keeps_each_parts_edges_with_the_group() {
    let store = Store::open_in_memory().unwrap();
    let mut recorder = Recorder::new(&[(0, 5), (5, 5)]);
    for i in 0..10 {
        recorder.page(i, &page(i + 1)).await;
    }
    recorder.keep(&store, WORK, "7", Some("Group Example"), 3);
    let kept = store.page_prints(WORK).unwrap();
    let pages: Vec<u32> = kept.iter().map(|p| p.page).collect();
    assert_eq!(pages, vec![0, 1, 3, 4, 5, 6, 8, 9]);
    assert!(kept.iter().all(|p| p.group_label.as_deref() == Some("Group Example")));
    assert!(kept.iter().any(|p| p.page == 4 && p.at_end));
}

#[test]
fn only_chapters_from_the_same_group_count() {
    let credit = PagePrint { page: 11, at_end: true, print: 0x5555_aaaa_5555_aaaa };
    let theirs = |chapter: &str, group: Option<&str>| KeptPrint {
        chapter: chapter.into(),
        page: 11,
        at_end: true,
        print: credit.print,
        group_label: group.map(str::to_owned),
    };
    let mine = [credit];
    // Two chapters of another group share the page: that group's credits, not this one's.
    let other = [theirs("1", Some("Group B")), theirs("2", Some("Group B"))];
    assert!(credit_pages(&other, &[], ("3", Some("Group A")), &mine).is_empty());
    let same = [theirs("1", Some("Group A")), theirs("2", Some("Group A"))];
    assert_eq!(credit_pages(&same, &[], ("3", Some("Group A")), &mine), mine);
    // A chapter whose group is not known counts with any.
    let unknown = [theirs("1", None), theirs("2", Some("Group B"))];
    assert_eq!(credit_pages(&unknown, &[], ("3", None), &mine), mine);
}
