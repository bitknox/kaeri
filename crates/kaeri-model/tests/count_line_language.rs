//! A count line in a language other than English.
//!
//! The language is the process's own, so this file holds one test, in a binary of its own: run
//! beside other tests, a switch of language would change the words they read.

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    browsing::count_line,
    catalog::{AiringStatus, Entry, FuzzyDate},
};

#[test]
fn a_start_date_is_written_in_the_order_the_language_writes_a_date() {
    let dated = Entry {
        status: Some(AiringStatus::NotYetReleased),
        start_date: Some(FuzzyDate { year: 2027, month: Some(1), day: Some(9) }),
        ..Entry::new(AnilistId::new(900_301), "Paper Crane Courier")
    };

    // Japanese writes the month first and marks the day: 1月9日, not 9 1月.
    assert_eq!(kaeri_i18n::set_language("ja"), "ja");
    let line = count_line(&dated).unwrap();
    assert_eq!(line.long, "1月9日開始");
    assert_eq!(line.short, "1月9日開始");
    assert_eq!(line.spoken, "1月9日開始");

    assert_eq!(kaeri_i18n::set_language("en"), "en");
    let line = count_line(&dated).unwrap();
    assert_eq!((line.long.as_str(), line.short.as_str()), ("Starts 9 Jan", "Starts 9 Jan"));
    assert_eq!(line.spoken, "Starts 9 January");
}
