//! Pure civil-date math on days-since-epoch (1970-01-01). Shared by the data
//! layer (catalog date bounds) and the handlers (filters, calendar, home).

pub(crate) fn current_utc_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .div_euclid(86_400) as i64
}

pub(crate) fn date_query_for_days(days: i64) -> String {
    let (year, month, day) = days_to_civil(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `month` must already be validated to 1..=12 by the caller.
pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => unreachable!("month out of range: {month}"),
    }
}

pub(crate) fn parse_ymd_to_days(created_at: &str) -> Option<i64> {
    let date_part = created_at.get(..10)?;
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: i32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let d: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(civil_to_days(y, m, d))
}

fn civil_to_days(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverse of `civil_to_days`. Howard Hinnant's algorithm.
/// Input is days since 1970-01-01; returns (year, month, day).
pub(crate) fn days_to_civil(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y } as i32;
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_to_civil_roundtrips() {
        for &(y, m, d) in &[
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 2, 29),
            (2025, 3, 1),
            (2026, 4, 22),
            (2099, 12, 31),
        ] {
            let days = civil_to_days(y, m, d);
            assert_eq!(days_to_civil(days), (y, m, d));
        }
    }

    #[test]
    fn parse_ymd_to_days_counts_days_and_rejects_garbage() {
        let a = parse_ymd_to_days("2024-01-01T00:00:00Z").unwrap();
        let b = parse_ymd_to_days("2024-01-15T00:00:00Z").unwrap();
        assert_eq!(b - a, 14);
        let c = parse_ymd_to_days("2025-01-01T00:00:00Z").unwrap();
        assert_eq!(c - a, 366); // 2024 is a leap year
        assert!(parse_ymd_to_days("bogus").is_none());
    }
}
