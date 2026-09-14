//! Dates as precise as a source made them.
//!
//! Sources disagree about what a date is. PubMed's `PubDate` is a year, a
//! year and a month written as `Jan` or `01`, a full date, a year and a
//! season, or free text such as `1998 Dec-1999 Jan`. [`PartialDate`] holds
//! each of those as what it is; [`Date`] is the calendar-valid case. Every
//! parser here accepts the forms observed in real data and nothing else
//! (plan §6): a form not listed is an error, and the fix is to list it.

use std::fmt;
use std::num::NonZeroU8;

/// A year, `1000..=2999`: wide enough for any scholarly record, narrow
/// enough to catch a dropped or doubled digit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Year(u16);

/// A month. `Option<Month>` is one byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum Month {
    January = 1,
    February = 2,
    March = 3,
    April = 4,
    May = 5,
    June = 6,
    July = 7,
    August = 8,
    September = 9,
    October = 10,
    November = 11,
    December = 12,
}

/// A day of the month, `1..=31`; calendar validity is [`Date::new`]'s job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(NonZeroU8);

/// A season, as PubMed writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

/// A quarter, as PubMed's `MedlineDate` writes it (`1st Quarter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
pub enum Quarter {
    First,
    Second,
    Third,
    Fourth,
}

/// A calendar-valid date. Four bytes; `Option<Date>` is four bytes too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: Year,
    month: Month,
    day: Day,
}

/// One end of a [`Span`]: a year, optionally to the month, optionally to
/// the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bound {
    /// The year.
    pub year: Year,
    /// The month, when given.
    pub month: Option<Month>,
    /// The day, when given; only with a month.
    pub day: Option<Day>,
}

/// A range of publication dates, from PubMed's `MedlineDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The start.
    pub start: Bound,
    /// The end, inclusive.
    pub end: Bound,
}

/// A publication date at whatever precision the source gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartialDate {
    /// Only the year.
    Year(Year),
    /// Year and month.
    YearMonth(Year, Month),
    /// A full date.
    Day(Date),
    /// Year and season.
    YearSeason(Year, Season),
    /// Year and quarter.
    YearQuarter(Year, Quarter),
    /// A range.
    Span(Span),
}

/// Why a date part did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
    /// A year is exactly four ASCII digits.
    YearDigits,
    /// A year is `1000..=2999`.
    YearRange,
    /// A month is `Jan`..`Dec` or a one- or two-digit number `1..=12`.
    Month,
    /// A day is a one- or two-digit number `1..=31`.
    Day,
    /// A season is `Winter`, `Spring`, `Summer` or `Fall`.
    Season,
    /// The day does not exist in that month of that year.
    Calendar,
    /// A date-time is `YYYY-MM-DDThh:mm:ssZ`.
    DateTimeShape,
    /// Hours, minutes or seconds out of range.
    Time,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::YearDigits => "year is not four digits",
            Self::YearRange => "year outside 1000..=2999",
            Self::Month => "month is not Jan..Dec or 1..12",
            Self::Day => "day is not 1..31",
            Self::Season => "season is not Winter, Spring, Summer or Fall",
            Self::Calendar => "day does not exist in that month",
            Self::DateTimeShape => "date-time is not YYYY-MM-DDThh:mm:ssZ",
            Self::Time => "time of day out of range",
        })
    }
}

impl std::error::Error for Error {}

/// Why a `MedlineDate` did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MedlineError {
    /// None of the recognized forms.
    Unrecognized,
    /// A recognized form with a bad part.
    Part(Error),
}

impl fmt::Display for MedlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unrecognized => f.write_str("not a recognized MedlineDate form"),
            Self::Part(e) => write!(f, "MedlineDate part: {e}"),
        }
    }
}

impl std::error::Error for MedlineError {}

impl From<Error> for MedlineError {
    fn from(e: Error) -> Self {
        Self::Part(e)
    }
}

/// Parses `1..=2` ASCII digits as a small number.
fn small_number(s: &str) -> Option<u8> {
    if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

impl Year {
    /// Parses exactly four digits.
    ///
    /// # Errors
    ///
    /// [`Error::YearDigits`] or [`Error::YearRange`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::YearDigits);
        }
        let value: u16 = s.parse().map_err(|_| Error::YearDigits)?;
        Self::new(value).ok_or(Error::YearRange)
    }

    /// Wraps a value in range.
    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        if value >= 1000 && value <= 2999 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// The year.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    const fn is_leap(self) -> bool {
        self.0.is_multiple_of(4) && (!self.0.is_multiple_of(100) || self.0.is_multiple_of(400))
    }
}

impl Month {
    const NAMES: [&'static str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    /// Parses `Jan`..`Dec` (PubMed's `PubDate`) or a one- or two-digit
    /// number (`ArticleDate`, `DateRevised`, and the history dates, which
    /// write both `1` and `01`).
    ///
    /// # Errors
    ///
    /// [`Error::Month`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        Self::from_name(s)
            .or_else(|| small_number(s).and_then(Self::from_number))
            .ok_or(Error::Month)
    }

    /// The month for a three-letter English name.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .position(|name| *name == s)
            .and_then(|i| Self::from_number(u8::try_from(i + 1).ok()?))
    }

    /// The month for `1..=12`.
    #[must_use]
    pub const fn from_number(n: u8) -> Option<Self> {
        Some(match n {
            1 => Self::January,
            2 => Self::February,
            3 => Self::March,
            4 => Self::April,
            5 => Self::May,
            6 => Self::June,
            7 => Self::July,
            8 => Self::August,
            9 => Self::September,
            10 => Self::October,
            11 => Self::November,
            12 => Self::December,
            _ => return None,
        })
    }

    /// `1..=12`.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// The three-letter English name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        Self::NAMES[self as usize - 1]
    }

    const fn days(self, year: Year) -> u8 {
        match self {
            Self::February => {
                if year.is_leap() {
                    29
                } else {
                    28
                }
            }
            Self::April | Self::June | Self::September | Self::November => 30,
            _ => 31,
        }
    }
}

impl Day {
    /// Parses a one- or two-digit number `1..=31`.
    ///
    /// # Errors
    ///
    /// [`Error::Day`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        small_number(s).and_then(Self::new).ok_or(Error::Day)
    }

    /// Wraps `1..=31`.
    #[must_use]
    pub const fn new(day: u8) -> Option<Self> {
        if day >= 1 && day <= 31 {
            match NonZeroU8::new(day) {
                Some(d) => Some(Self(d)),
                None => None,
            }
        } else {
            None
        }
    }

    /// `1..=31`.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0.get()
    }
}

impl Season {
    /// Parses PubMed's spelling.
    ///
    /// # Errors
    ///
    /// [`Error::Season`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        Self::from_name(s).ok_or(Error::Season)
    }

    /// The season for its name.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Winter" => Self::Winter,
            "Spring" => Self::Spring,
            "Summer" => Self::Summer,
            "Fall" => Self::Fall,
            _ => return None,
        })
    }
}

impl Quarter {
    /// The quarter for `1st Quarter`..`4th Quarter`.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "1st Quarter" => Self::First,
            "2nd Quarter" => Self::Second,
            "3rd Quarter" => Self::Third,
            "4th Quarter" => Self::Fourth,
            _ => return None,
        })
    }
}

impl Date {
    /// A calendar-valid date.
    ///
    /// # Errors
    ///
    /// [`Error::Calendar`] when the day does not exist in that month.
    pub const fn new(year: Year, month: Month, day: Day) -> Result<Self, Error> {
        if day.get() > month.days(year) {
            return Err(Error::Calendar);
        }
        Ok(Self { year, month, day })
    }

    /// The year.
    #[must_use]
    pub const fn year(self) -> Year {
        self.year
    }

    /// The month.
    #[must_use]
    pub const fn month(self) -> Month {
        self.month
    }

    /// The day.
    #[must_use]
    pub const fn day(self) -> Day {
        self.day
    }

    /// From integers as JSON sources write them.
    ///
    /// # Errors
    ///
    /// The part that is out of range, or [`Error::Calendar`].
    pub fn from_numbers(year: u32, month: u32, day: u32) -> Result<Self, Error> {
        let year = u16::try_from(year)
            .ok()
            .and_then(Year::new)
            .ok_or(Error::YearRange)?;
        let month = u8::try_from(month)
            .ok()
            .and_then(Month::from_number)
            .ok_or(Error::Month)?;
        let day = u8::try_from(day)
            .ok()
            .and_then(Day::new)
            .ok_or(Error::Day)?;
        Self::new(year, month, day)
    }

    /// Days since 1970-01-01, negative before it (Howard Hinnant's
    /// `days_from_civil`).
    #[must_use]
    pub const fn days_since_epoch(self) -> i64 {
        let m = self.month.number() as i64;
        let d = self.day.get() as i64;
        let y = self.year.get() as i64 - if m <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

/// A time of day to the minute with no zone, as PubMed's history dates
/// carry it. One byte each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
}

impl TimeOfDay {
    /// `0..=23` and `0..=59`.
    #[must_use]
    pub const fn new(hour: u8, minute: u8) -> Option<Self> {
        if hour <= 23 && minute <= 59 {
            Some(Self { hour, minute })
        } else {
            None
        }
    }

    /// The hour.
    #[must_use]
    pub const fn hour(self) -> u8 {
        self.hour
    }

    /// The minute.
    #[must_use]
    pub const fn minute(self) -> u8 {
        self.minute
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// An instant in UTC to the second, from `2014-07-05T15:19:03Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    date: Date,
    second_of_day: u32,
}

impl DateTime {
    /// Parses exactly `YYYY-MM-DDThh:mm:ssZ`.
    ///
    /// # Errors
    ///
    /// [`Error::DateTimeShape`], a bad date part, or [`Error::Time`].
    pub fn parse_utc(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 20
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
            || b[19] != b'Z'
        {
            return Err(Error::DateTimeShape);
        }
        let num = |range: std::ops::Range<usize>| -> Result<u32, Error> {
            let part = &s[range];
            if !part.bytes().all(|c| c.is_ascii_digit()) {
                return Err(Error::DateTimeShape);
            }
            part.parse().map_err(|_| Error::DateTimeShape)
        };
        let date = Date::from_numbers(num(0..4)?, num(5..7)?, num(8..10)?)?;
        let (h, m, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
        if h > 23 || m > 59 || sec > 59 {
            return Err(Error::Time);
        }
        Ok(Self {
            date,
            second_of_day: h * 3600 + m * 60 + sec,
        })
    }

    /// The calendar date.
    #[must_use]
    pub const fn date(self) -> Date {
        self.date
    }

    /// Seconds since the Unix epoch.
    #[must_use]
    pub const fn epoch_seconds(self) -> i64 {
        self.date.days_since_epoch() * 86_400 + self.second_of_day as i64
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.second_of_day;
        write!(
            f,
            "{}T{:02}:{:02}:{:02}Z",
            self.date,
            s / 3600,
            s / 60 % 60,
            s % 60
        )
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}",
            self.year.get(),
            self.month.number(),
            self.day.get()
        )
    }
}

/// `YYYY` at the front of `s`: four digits, or the shape is wrong.
fn take_year(s: &str) -> Result<(Year, &str), MedlineError> {
    if s.len() < 4 || !s.is_char_boundary(4) || !s[..4].bytes().all(|b| b.is_ascii_digit()) {
        return Err(MedlineError::Unrecognized);
    }
    let year = Year::parse(&s[..4])?;
    Ok((year, &s[4..]))
}

/// `Mon` at the front of `s`.
fn take_month(s: &str) -> Result<(Month, &str), MedlineError> {
    if s.len() < 3 || !s.is_char_boundary(3) {
        return Err(MedlineError::Unrecognized);
    }
    let month = Month::from_name(&s[..3]).ok_or(MedlineError::Unrecognized)?;
    Ok((month, &s[3..]))
}

/// `D` or `DD` at the front of `s`.
fn take_day(s: &str) -> Result<(Day, &str), MedlineError> {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return Err(MedlineError::Unrecognized);
    }
    let day = Day::parse(&s[..digits])?;
    Ok((day, &s[digits..]))
}

fn bound(year: Year, month: Option<Month>, day: Option<Day>) -> Result<Bound, MedlineError> {
    if let (Some(month), Some(day)) = (month, day) {
        Date::new(year, month, day)?;
    }
    Ok(Bound { year, month, day })
}

fn done(rest: &str) -> Result<(), MedlineError> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(MedlineError::Unrecognized)
    }
}

impl PartialDate {
    /// Parses PubMed's `MedlineDate` free text. The forms recognized, each
    /// observed in real data:
    ///
    /// | form                | example              |
    /// |---------------------|----------------------|
    /// | `YYYY Mon-Mon`      | `2026 Jan-Feb`       |
    /// | `YYYY Mon-YYYY Mon` | `1998 Dec-1999 Jan`  |
    /// | `YYYY Mon DD-DD`    | `1999 Dec 20-27`     |
    /// | `YYYY Mon DD-Mon DD`| `2001 Mar 30-Apr 1`  |
    /// | `YYYY-YYYY`         | `1976-1977`          |
    /// | `YYYY Season`       | `2000 Spring`        |
    /// | `YYYY Nth Quarter`  | `2025 1st Quarter`   |
    ///
    /// # Errors
    ///
    /// [`MedlineError::Unrecognized`] for any other shape, or a bad part.
    pub fn parse_medline(s: &str) -> Result<Self, MedlineError> {
        let (year, rest) = take_year(s)?;
        if let Some(rest) = rest.strip_prefix('-') {
            let (end, rest) = take_year(rest)?;
            done(rest)?;
            return Ok(Self::Span(Span {
                start: bound(year, None, None)?,
                end: bound(end, None, None)?,
            }));
        }
        let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
        if let Some(quarter) = Quarter::from_name(rest) {
            return Ok(Self::YearQuarter(year, quarter));
        }
        if let Some(season) = Season::from_name(rest) {
            return Ok(Self::YearSeason(year, season));
        }
        let (month, rest) = take_month(rest)?;
        if let Some(rest) = rest.strip_prefix('-') {
            // `-Mon` or `-YYYY Mon`
            let (end_year, end_month, rest) = if let Ok((end_year, rest)) = take_year(rest) {
                let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
                let (end_month, rest) = take_month(rest)?;
                (end_year, end_month, rest)
            } else {
                let (end_month, rest) = take_month(rest)?;
                (year, end_month, rest)
            };
            done(rest)?;
            return Ok(Self::Span(Span {
                start: bound(year, Some(month), None)?,
                end: bound(end_year, Some(end_month), None)?,
            }));
        }
        let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
        let (day, rest) = take_day(rest)?;
        let rest = rest.strip_prefix('-').ok_or(MedlineError::Unrecognized)?;
        // `-DD` or `-Mon DD`
        let (end_month, end_day, rest) = if let Ok((end_month, rest)) = take_month(rest) {
            let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
            let (end_day, rest) = take_day(rest)?;
            (end_month, end_day, rest)
        } else {
            let (end_day, rest) = take_day(rest)?;
            (month, end_day, rest)
        };
        done(rest)?;
        Ok(Self::Span(Span {
            start: bound(year, Some(month), Some(day))?,
            end: bound(year, Some(end_month), Some(end_day))?,
        }))
    }

    /// From integers as JSON sources write them: a year, optionally a
    /// month, optionally a day.
    ///
    /// # Errors
    ///
    /// The part that is out of range, or [`Error::Calendar`]; a day without
    /// a month is [`Error::Month`].
    pub fn from_numbers(year: u32, month: Option<u32>, day: Option<u32>) -> Result<Self, Error> {
        let y = u16::try_from(year)
            .ok()
            .and_then(Year::new)
            .ok_or(Error::YearRange)?;
        match (month, day) {
            (None, None) => Ok(Self::Year(y)),
            (Some(m), None) => {
                let m = u8::try_from(m)
                    .ok()
                    .and_then(Month::from_number)
                    .ok_or(Error::Month)?;
                Ok(Self::YearMonth(y, m))
            }
            (Some(m), Some(d)) => Ok(Self::Day(Date::from_numbers(year, m, d)?)),
            (None, Some(_)) => Err(Error::Month),
        }
    }

    /// A short label for the precision, for reports.
    #[must_use]
    pub const fn precision(&self) -> &'static str {
        match self {
            Self::Year(_) => "year",
            Self::YearMonth(..) => "year-month",
            Self::Day(_) => "day",
            Self::YearSeason(..) => "year-season",
            Self::YearQuarter(..) => "year-quarter",
            Self::Span(_) => "span",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bound, Date, DateTime, Day, Error, MedlineError, Month, PartialDate, Quarter, Season, Span,
        Year,
    };

    fn y(v: u16) -> Year {
        Year::new(v).unwrap()
    }

    fn d(v: u8) -> Day {
        Day::new(v).unwrap()
    }

    #[test]
    fn years_are_four_digits_in_range() {
        assert_eq!(Year::parse("2026").map(Year::get), Ok(2026));
        assert_eq!(Year::parse("226"), Err(Error::YearDigits));
        assert_eq!(Year::parse("20226"), Err(Error::YearDigits));
        assert_eq!(Year::parse("0999"), Err(Error::YearRange));
        assert_eq!(Year::parse("3000"), Err(Error::YearRange));
        assert_eq!(Year::parse(" 2026"), Err(Error::YearDigits));
    }

    #[test]
    fn months_take_names_and_both_number_widths() {
        assert_eq!(Month::parse("Jan"), Ok(Month::January));
        assert_eq!(Month::parse("Dec"), Ok(Month::December));
        assert_eq!(Month::parse("01"), Ok(Month::January));
        assert_eq!(Month::parse("1"), Ok(Month::January));
        assert_eq!(Month::parse("12"), Ok(Month::December));
        for bad in ["jan", "JAN", "January", "0", "13", "001", "", " 1"] {
            assert_eq!(Month::parse(bad), Err(Error::Month), "{bad:?}");
        }
        assert_eq!(Month::September.name(), "Sep");
        assert_eq!(Month::September.number(), 9);
    }

    #[test]
    fn days_and_calendar() {
        assert_eq!(Day::parse("01").map(Day::get), Ok(1));
        assert_eq!(Day::parse("31").map(Day::get), Ok(31));
        assert_eq!(Day::parse("0"), Err(Error::Day));
        assert_eq!(Day::parse("32"), Err(Error::Day));
        assert_eq!(Day::parse("001"), Err(Error::Day));
        assert!(Date::new(y(2024), Month::February, d(29)).is_ok());
        assert_eq!(
            Date::new(y(2023), Month::February, d(29)),
            Err(Error::Calendar)
        );
        assert!(Date::new(y(2000), Month::February, d(29)).is_ok());
        assert_eq!(
            Date::new(y(1900), Month::February, d(29)),
            Err(Error::Calendar)
        );
        assert_eq!(
            Date::new(y(2026), Month::April, d(31)),
            Err(Error::Calendar)
        );
        assert_eq!(
            Date::new(y(2026), Month::January, d(29))
                .unwrap()
                .to_string(),
            "2026-01-29"
        );
    }

    #[test]
    fn epoch_days_and_date_times() {
        assert_eq!(
            Date::from_numbers(1970, 1, 1).unwrap().days_since_epoch(),
            0
        );
        assert_eq!(
            Date::from_numbers(2000, 1, 1).unwrap().days_since_epoch(),
            10_957
        );
        assert_eq!(
            Date::from_numbers(2024, 2, 29).unwrap().days_since_epoch(),
            19_782
        );
        assert_eq!(
            Date::from_numbers(1969, 12, 31).unwrap().days_since_epoch(),
            -1
        );
        let dt = DateTime::parse_utc("2014-07-05T15:19:03Z").unwrap();
        assert_eq!(dt.epoch_seconds(), 1_404_573_543);
        assert_eq!(dt.to_string(), "2014-07-05T15:19:03Z");
        assert_eq!(
            DateTime::parse_utc("1928-10-01T00:00:00Z")
                .unwrap()
                .epoch_seconds(),
            -1_301_788_800
        );
        for bad in [
            "2014-07-05T15:19:03",
            "2014-07-05 15:19:03Z",
            "2014-07-05T24:00:00Z",
            "2014-13-05T15:19:03Z",
            "2014-07-05T15:19:60Z",
            "",
        ] {
            assert!(DateTime::parse_utc(bad).is_err(), "{bad}");
        }
        assert_eq!(
            PartialDate::from_numbers(2020, Some(3), None),
            Ok(PartialDate::YearMonth(y(2020), Month::March))
        );
        assert_eq!(
            PartialDate::from_numbers(2020, None, Some(3)),
            Err(Error::Month)
        );
        assert_eq!(
            PartialDate::from_numbers(2020, Some(13), None),
            Err(Error::Month)
        );
    }

    #[test]
    fn sizes_are_packed() {
        assert_eq!(size_of::<Date>(), 4);
        assert_eq!(size_of::<Option<Date>>(), 4);
        assert_eq!(size_of::<Option<Month>>(), 1);
        assert_eq!(size_of::<Option<Day>>(), 1);
        assert!(size_of::<PartialDate>() <= 12);
    }

    #[test]
    fn medline_forms_observed() {
        let span = |a: Bound, b: Bound| PartialDate::Span(Span { start: a, end: b });
        let b = |year: u16, month: Option<Month>, day: Option<u8>| Bound {
            year: y(year),
            month,
            day: day.map(d),
        };
        assert_eq!(
            PartialDate::parse_medline("2026 Jan-Dec"),
            Ok(span(
                b(2026, Some(Month::January), None),
                b(2026, Some(Month::December), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1998 Dec-1999 Jan"),
            Ok(span(
                b(1998, Some(Month::December), None),
                b(1999, Some(Month::January), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1999 Dec 20-27"),
            Ok(span(
                b(1999, Some(Month::December), Some(20)),
                b(1999, Some(Month::December), Some(27))
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("2001 Mar 30-Apr 1"),
            Ok(span(
                b(2001, Some(Month::March), Some(30)),
                b(2001, Some(Month::April), Some(1))
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1976-1977"),
            Ok(span(b(1976, None, None), b(1977, None, None)))
        );
        assert_eq!(
            PartialDate::parse_medline("2000 Spring"),
            Ok(PartialDate::YearSeason(y(2000), Season::Spring))
        );
        assert_eq!(
            PartialDate::parse_medline("2025 1st Quarter"),
            Ok(PartialDate::YearQuarter(y(2025), Quarter::First))
        );
    }

    #[test]
    fn medline_forms_not_yet_observed_fail() {
        for s in [
            "2026",
            "2026 Jan",
            "2026 Spring-Summer",
            "2026 Jan-",
            "2026 Jan-Dec ",
            "2026 jan-dec",
            "Jan-Dec 2026",
            "2026 Jan 1",
            "",
        ] {
            assert_eq!(
                PartialDate::parse_medline(s),
                Err(MedlineError::Unrecognized),
                "{s:?}"
            );
        }
        assert_eq!(
            PartialDate::parse_medline("2026 Feb 29-30"),
            Err(MedlineError::Part(Error::Calendar))
        );
        assert_eq!(
            PartialDate::parse_medline("0999 Jan-Dec"),
            Err(MedlineError::Part(Error::YearRange))
        );
    }
}
