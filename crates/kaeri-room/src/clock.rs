//! The clock check. strfry refuses ephemeral events more than 60 s old, so a device whose clock is
//! a minute behind can't send at all. Each room relay's NIP-11 answer (fetched with
//! `Cache-Control: no-cache`) gives an HTTP `Date`, minus any `Age`. The median offset is applied
//! only when at least two relays agree within 5 s and it's over 30 s: phones are NTP-synced, so
//! small offsets are noise, and one hostile or cached relay can't move the clock alone.

use std::time::Duration;

/// Offsets this small are noise.
pub const APPLY_OVER_MS: i64 = 30_000;
/// Two relays must agree this closely.
pub const AGREE_WITHIN_MS: i64 = 5_000;

/// The offset to apply from each relay's (`Date` − local clock), or `None` to leave the clock.
pub fn offset(samples: &[i64]) -> Option<i64> {
    if samples.len() < 2 {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2];
    let agree = sorted.iter().filter(|s| (**s - median).abs() <= AGREE_WITHIN_MS).count();
    (agree >= 2 && median.abs() > APPLY_OVER_MS).then_some(median)
}

/// An RFC 7231 date (`Sun, 06 Nov 1994 08:49:37 GMT`) as epoch seconds.
pub fn parse_http_date(text: &str) -> Option<i64> {
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = text.split_whitespace().skip(1);
    let day: i64 = parts.next()?.parse().ok()?;
    let name = parts.next()?;
    let month = MONTHS.iter().position(|m| *m == name)? as i64 + 1;
    let year: i64 = parts.next()?.parse().ok()?;
    let mut hms = parts.next()?.split(':').map(|p| p.parse::<i64>().ok());
    let (h, m, s) = (hms.next()??, hms.next()??, hms.next()??);
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + m * 60 + s)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A relay's NIP-11 address: `wss://` to `https://`, `ws://` to `http://`.
pub fn nip11_url(relay: &str) -> Option<String> {
    if let Some(rest) = relay.strip_prefix("wss://") {
        Some(format!("https://{rest}"))
    } else {
        relay.strip_prefix("ws://").map(|rest| format!("http://{rest}"))
    }
}

/// Ask one relay for its clock: (`Date` + `Age`) − local, in ms.
pub async fn sample(
    client: &reqwest::Client,
    relay: &str,
    now_ms: impl Fn() -> i64,
) -> Option<i64> {
    let url = nip11_url(relay)?;
    let asked = now_ms();
    let reply = client
        .get(url)
        .header("Accept", "application/nostr+json")
        .header("Cache-Control", "no-cache")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    let local = (asked + now_ms()) / 2;
    let headers = reply.headers();
    let date = parse_http_date(headers.get("date")?.to_str().ok()?)?;
    let age: i64 = headers
        .get("age")
        .and_then(|a| a.to_str().ok())
        .and_then(|a| a.parse().ok())
        .unwrap_or(0);
    // The Date header has one-second steps: count from the middle of that second.
    Some((date + age) * 1000 + 500 - local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_offsets_and_lone_relays_leave_the_clock() {
        assert_eq!(offset(&[]), None);
        assert_eq!(offset(&[90_000]), None);
        assert_eq!(offset(&[2_000, 1_000, -500]), None);
        assert_eq!(offset(&[-70_000, -71_000, 200]), Some(-70_000));
    }

    #[test]
    fn one_stale_or_hostile_relay_cant_move_the_clock() {
        // A CDN-cached Date an hour old on one relay: the other two hold.
        assert_eq!(offset(&[-3_600_000, 400, -300]), None);
        // Three that disagree with each other: nothing moves.
        assert_eq!(offset(&[-90_000, 0, 90_000]), None);
    }

    #[test]
    fn http_dates_read() {
        assert_eq!(parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT"), Some(784_111_777));
        assert_eq!(parse_http_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(parse_http_date("garbage"), None);
        assert_eq!(
            nip11_url("wss://relay.example/x").as_deref(),
            Some("https://relay.example/x")
        );
    }
}
