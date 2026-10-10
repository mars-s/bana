//! Calendar dates without a dependency: proleptic Gregorian, days since 1970-01-01.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A calendar day. Serialised as `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

impl Date {
    /// A valid date, or `None`.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        let ok = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month) && (-9999..=9999).contains(&year);
        ok.then_some(Date { year, month, day })
    }

    /// Today in the local time zone.
    /// A Unix time as local `YYYY-MM-DD HH:MM`.
    pub fn local_time(secs: u64) -> String {
        chrono::DateTime::from_timestamp(i64::try_from(secs).unwrap_or(0), 0)
            .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default()
    }

    /// Minutes since local midnight now.
    pub fn now_minutes() -> u16 {
        use chrono::Timelike;
        let t = chrono::Local::now().time();
        (t.hour() * 60 + t.minute()).min(1439) as u16
    }

    pub fn today() -> Date {
        use chrono::Datelike;
        let d = chrono::Local::now().date_naive();
        Date::new(d.year(), d.month() as u8, d.day() as u8).unwrap_or_else(|| Date::from_days(0))
    }

    /// Days since 1970-01-01 (Howard Hinnant's algorithm).
    pub fn to_days(self) -> i64 {
        let (m, d) = (self.month as i64, self.day as i64);
        let y = self.year as i64 - i64::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_days(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }

    pub fn add_days(self, n: i64) -> Date {
        Date::from_days(self.to_days().saturating_add(n))
    }

    /// 0 = Monday .. 6 = Sunday.
    pub fn weekday(self) -> usize {
        // 1970-01-01 was a Thursday (3).
        (self.to_days() + 3).rem_euclid(7) as usize
    }

    /// ISO 8601 week-numbering year and week (1..=53).
    pub fn iso_week(self) -> (i32, u32) {
        let thursday = self.add_days(3 - self.weekday() as i64);
        let jan1 = Date { year: thursday.year, month: 1, day: 1 };
        let week = (thursday.to_days() - jan1.to_days()) / 7 + 1;
        (thursday.year, week as u32)
    }

    /// `Sat, 10 Oct`: the title of a day node.
    pub fn day_title(self) -> String {
        let wd = WEEKDAYS.get(self.weekday()).copied().unwrap_or("");
        let mon = MONTHS.get(self.month as usize - 1).copied().unwrap_or("");
        format!("{wd}, {} {mon}", self.day)
    }

    /// A date (or a range of days) in words, relative to `today` (UC-01.08): `today`, `tomorrow`,
    /// `yesterday`, `friday` / `next friday` (the coming one), `last friday`, `next week`, `next
    /// month`, `in 3 days`, `in 2 weeks`, `Nov 21`, `21 November 2027`, `2026-11-21`, and any two
    /// of them joined by `to` or ` - ` (`Nov 21 to Nov 30`, `Nov 21 to 30`).
    pub fn from_words(text: &str, today: Date) -> Option<(Date, Option<Date>)> {
        let t = text.trim().to_lowercase();
        if let Some((a, b)) = t.split_once(" to ").or_else(|| t.split_once(" - ")) {
            let start = Date::one_from_words(a, today)?;
            // "Nov 21 to 30": the end takes the start's month and year.
            let end = match b.trim().parse::<u8>() {
                Ok(d) => Date::new(start.year, start.month, d)?,
                Err(_) => Date::one_from_words(b, today)?,
            };
            return (end > start).then_some((start, Some(end)));
        }
        Date::one_from_words(&t, today).map(|d| (d, None))
    }

    fn one_from_words(t: &str, today: Date) -> Option<Date> {
        let t = t.trim();
        match t {
            "today" | "now" => return Some(today),
            "tomorrow" => return Some(today.add_days(1)),
            "yesterday" => return Some(today.add_days(-1)),
            "next week" => return Some(today.add_days(7)),
            "last week" => return Some(today.add_days(-7)),
            "next month" | "last month" => {
                let (y, m) = if t == "next month" {
                    if today.month == 12 { (today.year + 1, 1) } else { (today.year, today.month + 1) }
                } else if today.month == 1 {
                    (today.year - 1, 12)
                } else {
                    (today.year, today.month - 1)
                };
                return Date::new(y, m, today.day.min(days_in_month(y, m)));
            }
            _ => {}
        }
        let words: Vec<&str> = t.split_whitespace().collect();
        let weekday = |w: &str| WEEKDAY_NAMES.iter().position(|n| w.len() >= 3 && n.starts_with(w));
        match words.as_slice() {
            [w] | ["next" | "this" | "on", w] if weekday(w).is_some() => {
                let want = weekday(w)? as i64;
                let ahead = (want - today.weekday() as i64).rem_euclid(7);
                return Some(today.add_days(if ahead == 0 { 7 } else { ahead }));
            }
            ["last", w] if weekday(w).is_some() => {
                let want = weekday(w)? as i64;
                let back = (today.weekday() as i64 - want).rem_euclid(7);
                return Some(today.add_days(-(if back == 0 { 7 } else { back })));
            }
            ["in", n, unit] => {
                let n: i64 = n.parse().ok()?;
                let days = match unit.trim_end_matches('s') {
                    "day" => n,
                    "week" => n * 7,
                    _ => return None,
                };
                return Some(today.add_days(days));
            }
            _ => {}
        }
        if let Some(d) = Date::parse(t).filter(|_| t.contains('-')) {
            return Some(d);
        }
        // Month and day in either order, an optional year; without a year, this year.
        let month = |w: &str| MONTH_NAMES.iter().position(|n| w.len() >= 3 && n.starts_with(w.trim_end_matches('.'))).map(|i| i as u8 + 1);
        let num = |w: &str| w.trim_end_matches(|c: char| c.is_alphabetic() || c == ',').parse::<i64>().ok();
        let (m, d, y) = match words.as_slice() {
            [a, b] => match (month(a), num(b), num(a), month(b)) {
                (Some(m), Some(d), _, _) | (_, _, Some(d), Some(m)) => (m, d, None),
                _ => return None,
            },
            [a, b, c] => match (month(a), num(b), num(a), month(b), num(c)) {
                (Some(m), Some(d), _, _, Some(y)) | (_, _, Some(d), Some(m), Some(y)) => (m, d, Some(y)),
                _ => return None,
            },
            _ => return None,
        };
        Date::new(i32::try_from(y.unwrap_or(i64::from(today.year))).ok()?, m, u8::try_from(d).ok()?)
    }

    /// Parses `YYYY-MM-DD`, `today`, `yesterday`, `tomorrow`.
    pub fn parse(s: &str) -> Option<Date> {
        let s = s.trim();
        match s.to_ascii_lowercase().as_str() {
            "today" => return Some(Date::today()),
            "yesterday" => return Some(Date::today().add_days(-1)),
            "tomorrow" => return Some(Date::today().add_days(1)),
            _ => {}
        }
        let mut it = s.splitn(3, '-');
        let y = it.next()?.parse().ok()?;
        let m = it.next()?.parse().ok()?;
        let d = it.next()?.parse().ok()?;
        Date::new(y, m, d)
    }
}

const WEEKDAY_NAMES: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
const MONTH_NAMES: [&str; 12] =
    ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl From<Date> for String {
    fn from(d: Date) -> String {
        d.to_string()
    }
}

impl TryFrom<String> for Date {
    type Error = String;
    fn try_from(s: String) -> Result<Date, String> {
        Date::parse(&s).ok_or_else(|| format!("bad date `{s}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_words() {
        // UC-01.08: today is Sat, 10 Oct 2026.
        let today = Date::new(2026, 10, 10).unwrap();
        let d = |t: &str| Date::from_words(t, today).map(|(a, b)| (a.to_string(), b.map(|b| b.to_string())));
        let one = |t: &str| d(t).map(|(a, _)| a);
        assert_eq!(one("tomorrow").as_deref(), Some("2026-10-11"));
        assert_eq!(one("Yesterday").as_deref(), Some("2026-10-09"));
        assert_eq!(one("next friday").as_deref(), Some("2026-10-16"));
        assert_eq!(one("fri").as_deref(), Some("2026-10-16"));
        assert_eq!(one("saturday").as_deref(), Some("2026-10-17"), "today's weekday means next week's");
        assert_eq!(one("last monday").as_deref(), Some("2026-10-05"));
        assert_eq!(one("in 3 days").as_deref(), Some("2026-10-13"));
        assert_eq!(one("in 2 weeks").as_deref(), Some("2026-10-24"));
        assert_eq!(one("next month").as_deref(), Some("2026-11-10"));
        assert_eq!(one("Nov 21").as_deref(), Some("2026-11-21"));
        assert_eq!(one("21 november 2027").as_deref(), Some("2027-11-21"));
        assert_eq!(one("2026-12-01").as_deref(), Some("2026-12-01"));
        assert_eq!(d("Nov 21 to Nov 30"), Some(("2026-11-21".into(), Some("2026-11-30".into()))));
        assert_eq!(d("Nov 21 to 30"), Some(("2026-11-21".into(), Some("2026-11-30".into()))));
        for no in ["", "garden", "nov", "Nov 31", "in many days", "Nov 30 to Nov 21", "friday the 13th please"] {
            assert_eq!(d(no), None, "{no}");
        }
    }

    #[test]
    fn round_trip_and_weekday() {
        let d = Date::new(2026, 10, 10).unwrap();
        assert_eq!(Date::from_days(d.to_days()), d);
        assert_eq!(d.weekday(), 5); // Saturday
        assert_eq!(d.day_title(), "Sat, 10 Oct");
        assert_eq!(d.iso_week(), (2026, 41));
        assert_eq!(Date::from_days(0), Date::new(1970, 1, 1).unwrap());
    }

    #[test]
    fn iso_week_edges() {
        assert_eq!(Date::new(2024, 12, 30).unwrap().iso_week(), (2025, 1));
        assert_eq!(Date::new(2021, 1, 3).unwrap().iso_week(), (2020, 53));
        assert_eq!(Date::new(2024, 2, 29).unwrap().add_days(1), Date::new(2024, 3, 1).unwrap());
    }

    #[test]
    fn parse_rejects_junk() {
        assert!(Date::parse("2026-02-30").is_none());
        assert!(Date::parse("x").is_none());
        assert_eq!(Date::parse("2026-1-2"), Date::new(2026, 1, 2));
    }
}
