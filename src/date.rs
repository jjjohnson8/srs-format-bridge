//! Just enough proleptic Gregorian calendar arithmetic to add a number of
//! days to an ISO "YYYY-MM-DD" date, which is all the scheduler side needs
//! to turn a review interval into a due date. Uses Howard Hinnant's
//! days-from-civil algorithm so it stays exact across month/year/leap-year
//! boundaries without pulling in a date library:
//! <https://howardhinnant.github.io/date_algorithms.html>

pub fn add_days(date: &str, days: i64) -> Result<String, String> {
    let (y, m, d) = parse(date)?;
    let (y, m, d) = civil_from_days(days_from_civil(y, m, d) + days);
    Ok(format!("{y:04}-{m:02}-{d:02}"))
}

fn parse(date: &str) -> Result<(i64, i64, i64), String> {
    let invalid = || format!("'{date}' is not a YYYY-MM-DD date");
    if !date.is_ascii() || date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return Err(invalid());
    }
    let year = date[0..4].parse::<i64>().map_err(|_| invalid())?;
    let month = date[5..7].parse::<i64>().map_err(|_| invalid())?;
    let day = date[8..10].parse::<i64>().map_err(|_| invalid())?;
    if !(1..=12).contains(&month) {
        return Err(format!("'{date}' has an invalid month"));
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(format!("'{date}' has an invalid day"));
    }
    Ok((year, month, day))
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        _ => 28,
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_days_within_a_month() {
        assert_eq!(add_days("2026-09-18", 3).unwrap(), "2026-09-21");
    }

    #[test]
    fn adds_days_across_a_month_boundary() {
        assert_eq!(add_days("2026-09-28", 5).unwrap(), "2026-10-03");
    }

    #[test]
    fn adds_days_across_a_year_boundary() {
        assert_eq!(add_days("2026-12-30", 3).unwrap(), "2027-01-02");
    }

    #[test]
    fn adds_days_across_february_in_a_leap_year() {
        assert_eq!(add_days("2028-02-28", 1).unwrap(), "2028-02-29");
    }

    #[test]
    fn adds_days_across_february_in_a_non_leap_year() {
        assert_eq!(add_days("2026-02-28", 1).unwrap(), "2026-03-01");
    }

    #[test]
    fn zero_days_returns_the_same_date() {
        assert_eq!(add_days("2026-09-18", 0).unwrap(), "2026-09-18");
    }

    #[test]
    fn rejects_malformed_dates() {
        assert!(add_days("2026/09/18", 1).is_err());
        assert!(add_days("26-09-18", 1).is_err());
        assert!(add_days("not-a-date", 1).is_err());
    }

    #[test]
    fn rejects_calendar_dates_that_do_not_exist() {
        assert!(add_days("2026-02-29", 1).is_err());
        assert!(add_days("2026-13-01", 1).is_err());
        assert!(add_days("2026-04-31", 1).is_err());
    }
}
