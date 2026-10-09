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

    /// Today in UTC.
    // ponytail: UTC, not the local zone; near midnight "today" can be off by one. Add a tz crate if that bites.
    pub fn today() -> Date {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        Date::from_days((secs / 86_400) as i64)
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
