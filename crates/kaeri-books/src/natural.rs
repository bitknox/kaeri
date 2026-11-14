//! Page order. Archives list entries in whatever order they were added, so pages are sorted by
//! name the way people number files: `p2` before `p10`, case ignored.

use std::cmp::Ordering;

/// Compares two names with runs of digits compared as numbers and the rest compared without case.
pub fn compare(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.as_bytes(), b.as_bytes());
    loop {
        match (x.first(), y.first()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let (n, rest_x) = split_digits(x);
                let (m, rest_y) = split_digits(y);
                let ord = compare_numbers(n, m);
                if ord != Ordering::Equal {
                    return ord;
                }
                x = rest_x;
                y = rest_y;
            }
            (Some(c), Some(d)) => {
                let ord = c.to_ascii_lowercase().cmp(&d.to_ascii_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                x = &x[1..];
                y = &y[1..];
            }
        }
    }
}

fn split_digits(s: &[u8]) -> (&[u8], &[u8]) {
    let end = s.iter().position(|c| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

fn compare_numbers(a: &[u8], b: &[u8]) -> Ordering {
    let strip = |s: &[u8]| -> usize { s.iter().position(|&c| c != b'0').unwrap_or(s.len()) };
    let (a, b) = (&a[strip(a)..], &b[strip(b)..]);
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_sort_as_numbers() {
        let mut v = vec!["p10.jpg", "p2.jpg", "P1.jpg", "p02a.jpg", "cover.jpg", "p002.jpg"];
        v.sort_by(|a, b| compare(a, b));
        assert_eq!(v, ["cover.jpg", "P1.jpg", "p002.jpg", "p2.jpg", "p02a.jpg", "p10.jpg"]);
    }

    #[test]
    fn long_digit_runs_do_not_overflow() {
        let a = format!("p{}1", "9".repeat(50));
        let b = format!("p{}2", "9".repeat(50));
        assert_eq!(compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn folders_sort_before_their_contents_order() {
        let mut v = vec!["ch10/01.jpg", "ch2/01.jpg", "ch2/10.jpg", "ch2/9.jpg"];
        v.sort_by(|a, b| compare(a, b));
        assert_eq!(v, ["ch2/01.jpg", "ch2/9.jpg", "ch2/10.jpg", "ch10/01.jpg"]);
    }
}
