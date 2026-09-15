//! Wall-clock time as text, without a date crate: receipts say when a
//! transfer began and listings are named by the day they were taken.

use std::time::{SystemTime, UNIX_EPOCH};

/// Now in UTC, to the second: `2026-09-15T08:08:00Z`.
#[must_use]
pub fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (date, rest) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil(i64::try_from(date).unwrap_or(0));
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Today in UTC: `2026-09-15`.
#[must_use]
pub fn today() -> String {
    now()[..10].to_owned()
}

/// Days since 1970-01-01 to a civil date, proleptic Gregorian — Howard
/// Hinnant's algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 1..=12, 1..=31
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::{civil, now, today};

    #[test]
    fn civil_dates_are_right_around_the_edges() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(10_957), (2000, 1, 1));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(20_711), (2026, 9, 15));
        assert_eq!(civil(-719_468), (0, 3, 1));
    }

    #[test]
    fn now_and_today_have_the_shape() {
        let now = now();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z') && &now[10..11] == "T", "{now}");
        assert!(now.starts_with("20"), "{now}");
        assert_eq!(today(), &now[..10]);
    }
}
