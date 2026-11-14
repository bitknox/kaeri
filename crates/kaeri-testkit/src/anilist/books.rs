//! The fake's books: AniList's `MANGA` type, answered from hand-written records with made-up
//! titles ([`crate::fixtures::books`] and whatever a test adds). Pages filter and sort as AniList
//! does for the arguments the app sends; several under aliases are answered each as alone.

use serde_json::{Value, json};

use super::Reply;

/// Answer a books request: one root, or several under `q0`, `q1` and on. `shift` is how far the
/// fake's clock is from the fixtures' day, so a start "in the last 12 months" means the same
/// books whatever day a test runs.
pub(super) fn respond(books: &[Value], document: &str, vars: &Value, shift: i64) -> Reply {
    match super::together(document, vars) {
        Some(parts) => {
            let mut data = serde_json::Map::new();
            for (alias, document, vars) in parts {
                let mut answer = one(books, &document, &vars, shift);
                let root = super::root_field(&document);
                data.insert(alias, answer[&root].take());
            }
            Reply::ok(Value::Object(data))
        }
        None if document.contains("Media(id:") && book(books, &vars["id"]).is_none() => {
            Reply::errors(404, "Not Found.", json!({ "Media": null }))
        }
        None => Reply::ok(one(books, document, vars, shift)),
    }
}

fn book<'a>(books: &'a [Value], id: &Value) -> Option<&'a Value> {
    books.iter().find(|b| b["id"] == *id)
}

/// A list's book: what a page asks for, without the page-only fields.
fn listed(book: &Value) -> Value {
    let mut book = book.clone();
    if let Some(fields) = book.as_object_mut() {
        for heavy in ["staff", "relations", "recommendations"] {
            fields.remove(heavy);
        }
    }
    book
}

/// A book's start as AniList's `YYYYMMDD` integer, a missing month or day as nought.
fn started(book: &Value) -> i64 {
    let part = |k: &str| book["startDate"][k].as_i64().unwrap_or_default();
    part("year") * 10_000 + part("month") * 100 + part("day")
}

/// A `YYYYMMDD` start moved back by the clock's shift, to the fixtures' calendar.
fn on_fixture_day(date: i64, shift: i64) -> i64 {
    let (year, month, day) = (date / 10_000, date / 100 % 100, date % 100);
    let Ok(date) = jiff::civil::Date::new(year as i16, month as i8, day as i8) else {
        return date;
    };
    let days = shift.div_euclid(86_400);
    let moved = date.checked_sub(jiff::Span::new().days(days)).unwrap_or(date);
    i64::from(moved.year()) * 10_000 + i64::from(moved.month()) * 100 + i64::from(moved.day())
}

fn one(books: &[Value], document: &str, vars: &Value, shift: i64) -> Value {
    // The count of shows a reading search carries beside its books: the fake has none to
    // count in a books request.
    if document.contains("type: ANIME") {
        return json!({ "Page": { "pageInfo": { "total": 0 }, "media": [] } });
    }
    if document.contains("Media(id:") {
        return json!({ "Media": book(books, &vars["id"]).cloned() });
    }
    let text = |v: &Value| v.as_str().map(str::to_lowercase);
    let list =
        |name: &str| -> Vec<Value> { vars[name].as_array().cloned().unwrap_or_default() };
    let term = text(&vars["search"]);
    let mut found: Vec<&Value> = books
        .iter()
        .filter(|b| {
            let names: Vec<String> = ["romaji", "english", "native"]
                .iter()
                .filter_map(|k| text(&b["title"][*k]))
                .chain(b["synonyms"].as_array().into_iter().flatten().filter_map(text))
                .collect();
            let genres = b["genres"].as_array().cloned().unwrap_or_default();
            let tags: Vec<Value> =
                b["tags"].as_array().into_iter().flatten().map(|t| t["name"].clone()).collect();
            let country = &b["countryOfOrigin"];
            term.as_ref().is_none_or(|t| names.iter().any(|n| n.contains(t.as_str())))
                && (vars["id_in"].is_null() || list("id_in").contains(&b["id"]))
                && (vars["format_in"].is_null() || list("format_in").contains(&b["format"]))
                && (vars["status_in"].is_null() || list("status_in").contains(&b["status"]))
                && (vars["countryOfOrigin"].is_null() || vars["countryOfOrigin"] == *country)
                && (vars["countryOfOrigin_in"].is_null()
                    || list("countryOfOrigin_in").contains(country))
                && list("genre_in").iter().all(|g| genres.contains(g))
                && !list("genre_not_in").iter().any(|g| genres.contains(g))
                && list("tag_in").iter().all(|t| tags.contains(t))
                && !list("tag_not_in").iter().any(|t| tags.contains(t))
                && (vars["isAdult"].is_null() || vars["isAdult"] == b["isAdult"])
                && vars["startDate_greater"]
                    .as_i64()
                    .is_none_or(|after| started(b) > on_fixture_day(after, shift))
        })
        .collect();
    let sort = list("sort").first().and_then(|s| s.as_str().map(str::to_owned));
    let number = |b: &Value, k: &str| b[k].as_i64().unwrap_or_default();
    match sort.as_deref() {
        Some("POPULARITY_DESC" | "TRENDING_DESC") => {
            found.sort_by_key(|b| std::cmp::Reverse(number(b, "popularity")));
        }
        Some("SCORE_DESC") => {
            found.sort_by_key(|b| std::cmp::Reverse(number(b, "averageScore")))
        }
        Some("START_DATE_DESC") => {
            found.sort_by_key(|b| std::cmp::Reverse(number(&b["startDate"], "year")));
        }
        Some("TITLE_ROMAJI") => {
            found.sort_by_key(|b| b["title"]["romaji"].as_str().unwrap_or_default().to_owned());
        }
        _ => {}
    }
    let per_page = vars["perPage"].as_u64().unwrap_or(50).clamp(1, 50) as usize;
    let page = vars["page"].as_u64().unwrap_or(1).max(1) as usize;
    let from = (page - 1) * per_page;
    let media: Vec<Value> = found.iter().skip(from).take(per_page).map(|b| listed(b)).collect();
    let has_next = found.len() > from + per_page;
    json!({ "Page": { "pageInfo": { "hasNextPage": has_next }, "media": media } })
}
