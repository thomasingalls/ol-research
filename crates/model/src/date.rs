//! Dates as precise as a source made them.
//!
//! Sources disagree about what a date is. PubMed's `PubDate` is a year, a
//! year and a month written as `Jan` or `01`, a full date, a year and a
//! season, or free text such as `1998 Dec-1999 Jan`. [`PartialDate`] holds
//! each of those as what it is; [`Date`] is the calendar-valid case. Every
//! parser here accepts the forms observed in real data and nothing else
//! (plan §6): a form not listed is an error, and the fix is to list it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::num::NonZeroU8;

/// A year, `1000..=2999`: wide enough for any scholarly record, narrow
/// enough to catch a dropped or doubled digit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Year(u16);

/// A month. `Option<Month>` is one byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Day(NonZeroU8);

/// A season, as PubMed writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

/// A quarter, as PubMed's `MedlineDate` writes it (`1st Quarter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bound {
    /// The year.
    pub year: Year,
    /// The month, when given.
    pub month: Option<Month>,
    /// The day, when given; only with a month.
    pub day: Option<Day>,
}

/// A range of publication dates, from PubMed's `MedlineDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// The start.
    pub start: Bound,
    /// The end, inclusive.
    pub end: Bound,
}

/// A seasonal volume: the years a volume spans with the season or seasons
/// its issue names — `1976-1977 Winter`, `1977-1978 Fall-Winter`, both
/// pubmed26n0001. Exactly the two facts the source states, with no guess
/// about which of the two years a season falls in; `from_year == to_year`
/// and `from == to` when the source names one of either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seasons {
    /// The first year named.
    pub from_year: Year,
    /// The last year named.
    pub to_year: Year,
    /// The first season named.
    pub from: Season,
    /// The last season named.
    pub to: Season,
}

/// A publication date at whatever precision the source gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// A seasonal volume, over one year or two.
    Seasons(Seasons),
    /// A range.
    Span(Span),
}

/// Why a date part did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// A date-time with milliseconds is `YYYY-MM-DDThh:mm:ss.fffZ`.
    DateTimeMillisShape,
    /// A naive date-time is `YYYY-MM-DDThh:mm:ss`, with an optional
    /// fraction of one to six digits and no zone.
    DateTimeNaiveShape,
    /// A date is `YYYY-MM-DD`.
    DateShape,
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
            Self::DateTimeMillisShape => "date-time is not YYYY-MM-DDThh:mm:ss.fffZ",
            Self::DateTimeNaiveShape => {
                "date-time is not YYYY-MM-DDThh:mm:ss with an optional fraction and no zone"
            }
            Self::DateShape => "date is not YYYY-MM-DD",
            Self::Time => "time of day out of range",
        })
    }
}

impl std::error::Error for Error {}

/// Why a `MedlineDate` did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    /// The month for a three-letter English name, in any case: PubMed's
    /// oldest records shout them (`1975 MAR-APR`, `1977 JAN-FEB`) and
    /// sometimes only half of one (`1976 Jul-AUG`, pubmed26n0001).
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .position(|name| name.eq_ignore_ascii_case(s))
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
            // `Autumn` on 2 of 15,432 PubMed records looked up 2026-09-15.
            "Fall" | "Autumn" => Self::Fall,
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

    /// Parses exactly `YYYY-MM-DD`, as OpenAlex writes `publication_date`.
    ///
    /// # Errors
    ///
    /// [`Error::DateShape`], then as [`Self::from_numbers`].
    pub fn parse_iso(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return Err(Error::DateShape);
        }
        let num = |range: std::ops::Range<usize>| -> Result<u32, Error> {
            let part = &s[range];
            if !part.bytes().all(|c| c.is_ascii_digit()) {
                return Err(Error::DateShape);
            }
            part.parse().map_err(|_| Error::DateShape)
        };
        Self::from_numbers(num(0..4)?, num(5..7)?, num(8..10)?)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// An instant in UTC to the millisecond, from `2014-07-05T15:19:03Z`
/// (Crossref) or `2025-07-25T07:35:27.710Z` (OpenAlex). Eight bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    date: Date,
    millis_of_day: u32,
}

impl DateTime {
    /// Parses exactly `YYYY-MM-DDThh:mm:ssZ`.
    ///
    /// # Errors
    ///
    /// [`Error::DateTimeShape`], a bad date part, or [`Error::Time`].
    pub fn parse_utc(s: &str) -> Result<Self, Error> {
        Self::parse(s, false)
    }

    /// Parses exactly `YYYY-MM-DDThh:mm:ss.fffZ`.
    ///
    /// # Errors
    ///
    /// [`Error::DateTimeMillisShape`], a bad date part, or [`Error::Time`].
    pub fn parse_utc_millis(s: &str) -> Result<Self, Error> {
        Self::parse(s, true)
    }

    /// Parses `YYYY-MM-DDThh:mm:ss` with an optional `.` and one to six
    /// fraction digits, and no zone designator — the form OpenAlex's API
    /// writes (2026-09), UTC by its documentation — keeping the fraction
    /// to the millisecond.
    ///
    /// # Errors
    ///
    /// [`Error::DateTimeNaiveShape`], a bad date part, or [`Error::Time`].
    pub fn parse_utc_naive(s: &str) -> Result<Self, Error> {
        let shape = Error::DateTimeNaiveShape;
        let (head, fraction) = match s.split_once('.') {
            Some((head, fraction)) => (head, Some(fraction)),
            None => (s, None),
        };
        let millis = match fraction {
            None => 0,
            Some(digits)
                if (1..=6).contains(&digits.len())
                    && digits.bytes().all(|c| c.is_ascii_digit()) =>
            {
                // The first three digits, right-padded: "5" is 500 ms.
                digits
                    .bytes()
                    .chain(std::iter::repeat(b'0'))
                    .take(3)
                    .fold(0, |n, c| n * 10 + u32::from(c - b'0'))
            }
            Some(_) => return Err(shape),
        };
        let (date, seconds) = Self::parse_head(head, shape)?;
        Ok(Self {
            date,
            millis_of_day: seconds * 1000 + millis,
        })
    }

    fn parse(s: &str, fraction: bool) -> Result<Self, Error> {
        let shape = if fraction {
            Error::DateTimeMillisShape
        } else {
            Error::DateTimeShape
        };
        let b = s.as_bytes();
        let len = if fraction { 24 } else { 20 };
        if b.len() != len || b[len - 1] != b'Z' || (fraction && b[19] != b'.') {
            return Err(shape);
        }
        let (date, seconds) = Self::parse_head(&s[..19], shape)?;
        let millis = if fraction {
            let part = &s[20..23];
            if !part.bytes().all(|c| c.is_ascii_digit()) {
                return Err(shape);
            }
            part.parse().map_err(|_| shape)?
        } else {
            0
        };
        Ok(Self {
            date,
            millis_of_day: seconds * 1000 + millis,
        })
    }

    /// `YYYY-MM-DDThh:mm:ss`, exactly, as the date and the second of the
    /// day; `shape` is the error for anything else.
    fn parse_head(head: &str, shape: Error) -> Result<(Date, u32), Error> {
        let b = head.as_bytes();
        if b.len() != 19
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
        {
            return Err(shape);
        }
        let num = |range: std::ops::Range<usize>| -> Result<u32, Error> {
            let part = &head[range];
            if !part.bytes().all(|c| c.is_ascii_digit()) {
                return Err(shape);
            }
            part.parse().map_err(|_| shape)
        };
        let date = Date::from_numbers(num(0..4)?, num(5..7)?, num(8..10)?)?;
        let (h, m, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
        if h > 23 || m > 59 || sec > 59 {
            return Err(Error::Time);
        }
        Ok((date, h * 3600 + m * 60 + sec))
    }

    /// The calendar date.
    #[must_use]
    pub const fn date(self) -> Date {
        self.date
    }

    /// Seconds since the Unix epoch, rounded down.
    #[must_use]
    pub const fn epoch_seconds(self) -> i64 {
        self.date.days_since_epoch() * 86_400 + (self.millis_of_day / 1000) as i64
    }

    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub const fn epoch_millis(self) -> i64 {
        self.date.days_since_epoch() * 86_400_000 + self.millis_of_day as i64
    }

    /// The sub-second part, `0..=999`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // below 1000
    pub const fn millis(self) -> u16 {
        (self.millis_of_day % 1000) as u16
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.millis_of_day / 1000;
        write!(
            f,
            "{}T{:02}:{:02}:{:02}",
            self.date,
            s / 3600,
            s / 60 % 60,
            s % 60
        )?;
        if !self.millis_of_day.is_multiple_of(1000) {
            write!(f, ".{:03}", self.millis_of_day % 1000)?;
        }
        f.write_str("Z")
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

/// The months' full English names, which `MedlineDate` spells now and
/// then (`2019 May/June`, pubmed26n1339).
const FULL_MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `Mon`, or the month's full name, at the front of `s`.
fn take_month(s: &str) -> Result<(Month, &str), MedlineError> {
    for (number, name) in (1u8..).zip(FULL_MONTHS) {
        // Case-insensitively, for the same reason `Month::from_name` is.
        if s.len() >= name.len()
            && s.is_char_boundary(name.len())
            && s[..name.len()].eq_ignore_ascii_case(name)
            && let Some(month) = Month::from_number(number)
        {
            return Ok((month, &s[name.len()..]));
        }
    }
    if s.len() < 3 || !s.is_char_boundary(3) {
        return Err(MedlineError::Unrecognized);
    }
    let month = Month::from_name(&s[..3]).ok_or(MedlineError::Unrecognized)?;
    Ok((month, &s[3..]))
}

/// A season's name at the front of `s`.
fn take_season(s: &str) -> Result<(Season, &str), MedlineError> {
    let end = s
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    let season = Season::from_name(&s[..end]).ok_or(MedlineError::Unrecognized)?;
    Ok((season, &s[end..]))
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

/// The separator of a range that starts with a month: `-`, and the
/// spellings the update files pubmed26n1335 to 1339 showed beside it, ` - `
/// (`2020 May - Jun`), `/` (`2021 Mar/Apr`) and ` & ` (`2020 Feb & Mar`,
/// an issue for both months).
fn month_range(rest: &str) -> Option<&str> {
    rest.strip_prefix('-')
        .or_else(|| rest.strip_prefix(" - "))
        .or_else(|| rest.strip_prefix('/'))
        .or_else(|| rest.strip_prefix(" & "))
}

impl PartialDate {
    /// Parses PubMed's `MedlineDate` free text. The forms recognized, each
    /// observed in real data:
    ///
    /// | form                     | example                  |
    /// |--------------------------|--------------------------|
    /// | `YYYY`                   | `2022`                   |
    /// | `YYYY Mon.`              | `2017 Feb.`              |
    /// | `YYYY Mon-Mon`           | `2026 Jan-Feb`           |
    /// | `YYYY Mon - Mon`         | `2020 May - Jun`         |
    /// | `YYYY Mon/Mon`           | `2021 Mar/Apr`           |
    /// | `YYYY Mon & Mon`         | `2020 Feb & Mar`         |
    /// | `YYYY Mon-YYYY Mon`      | `1998 Dec-1999 Jan`      |
    /// | `YYYY Mon DD-DD`         | `1999 Dec 20-27`         |
    /// | `YYYY Mon DD-Mon DD`     | `2001 Mar 30-Apr 1`      |
    /// | `YYYY Mon DD-YYYY Mon DD`| `2000 Dec 8-2001 Jan 10` |
    /// | `YYYY-YYYY`              | `1976-1977`              |
    /// | `YYYY Season`            | `2000 Spring`            |
    /// | `YYYY Nth Quarter`       | `2025 1st Quarter`       |
    /// | `YYYY-YYYY Season`       | `1976-1977 Winter`       |
    /// | `YYYY-YYYY Season-Season`| `1977-1978 Fall-Winter`  |
    /// | `YYYY Season-Season`     | `1978 Fall-Winter`       |
    ///
    /// The last three are [`PartialDate::Seasons`].
    ///
    /// A range that starts with a month takes any of its four separators
    /// wherever the plain `-` is allowed, and a month may be spelled out in
    /// full (`2019 May/June`).
    ///
    /// # Errors
    ///
    /// [`MedlineError::Unrecognized`] for any other shape, or a bad part.
    pub fn parse_medline(s: &str) -> Result<Self, MedlineError> {
        let (year, rest) = take_year(s)?;
        if rest.is_empty() {
            return Ok(Self::Year(year));
        }
        if let Some(rest) = rest.strip_prefix('-') {
            let (end, rest) = take_year(rest)?;
            if let Some(rest) = rest.strip_prefix(' ') {
                // `YYYY-YYYY Season` or `YYYY-YYYY Season-Season`: a
                // seasonal volume, which the years alone would lose.
                let (from, rest) = take_season(rest)?;
                let to = if let Some(rest) = rest.strip_prefix('-') {
                    let (to, rest) = take_season(rest)?;
                    done(rest)?;
                    to
                } else {
                    done(rest)?;
                    from
                };
                return Ok(Self::Seasons(Seasons {
                    from_year: year,
                    to_year: end,
                    from,
                    to,
                }));
            }
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
        // `YYYY Season-Season`, one year's seasonal issue spanning two of
        // them: `1978 Fall-Winter`, pubmed26n0002.
        if let Ok((from, after)) = take_season(rest)
            && let Some(after) = after.strip_prefix('-')
            && let Ok((to, after)) = take_season(after)
            && done(after).is_ok()
        {
            return Ok(Self::Seasons(Seasons {
                from_year: year,
                to_year: year,
                from,
                to,
            }));
        }
        let (month, rest) = take_month(rest)?;
        if rest == "." {
            return Ok(Self::YearMonth(year, month));
        }
        if let Some(rest) = month_range(rest) {
            // `-Mon` or `-YYYY Mon`, either with an optional ` DD` on the
            // end month: `2026 Jan-Feb 01`, 56 records of one journal in
            // pubmed26n1333.
            let (end_year, end_month, rest) = if let Ok((end_year, rest)) = take_year(rest) {
                let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
                let (end_month, rest) = take_month(rest)?;
                (end_year, end_month, rest)
            } else {
                let (end_month, rest) = take_month(rest)?;
                (year, end_month, rest)
            };
            let (end_day, rest) = match rest.strip_prefix(' ') {
                Some(rest) => {
                    let (day, rest) = take_day(rest)?;
                    (Some(day), rest)
                }
                None => (None, rest),
            };
            done(rest)?;
            return Ok(Self::Span(Span {
                start: bound(year, Some(month), None)?,
                end: bound(end_year, Some(end_month), end_day)?,
            }));
        }
        let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
        let (day, rest) = take_day(rest)?;
        let rest = rest.strip_prefix('-').ok_or(MedlineError::Unrecognized)?;
        // `-DD`, `-Mon DD`, or `-YYYY Mon DD` (`2000 Dec 8-2001 Jan 10`,
        // pubmed26n1339): a year is four digits and a day at most two.
        let (end_year, end_month, end_day, rest) = if let Ok((end_year, rest)) = take_year(rest) {
            let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
            let (end_month, rest) = take_month(rest)?;
            let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
            let (end_day, rest) = take_day(rest)?;
            (end_year, end_month, end_day, rest)
        } else if let Ok((end_month, rest)) = take_month(rest) {
            let rest = rest.strip_prefix(' ').ok_or(MedlineError::Unrecognized)?;
            let (end_day, rest) = take_day(rest)?;
            (year, end_month, end_day, rest)
        } else {
            let (end_day, rest) = take_day(rest)?;
            (year, month, end_day, rest)
        };
        done(rest)?;
        Ok(Self::Span(Span {
            start: bound(year, Some(month), Some(day))?,
            end: bound(end_year, Some(end_month), Some(end_day))?,
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
            Self::Seasons(_) => "seasons",
            Self::Span(_) => "span",
        }
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, (self.year, self.month, self.day), serializer)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "date",
            |(year, month, day): (Year, Month, Day)| Self::new(year, month, day).ok(),
            |s| Self::parse_iso(s).ok(),
        )
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, (self.date, self.millis_of_day), serializer)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "date-time",
            |(date, millis_of_day): (Date, u32)| {
                (millis_of_day < 86_400_000).then_some(Self {
                    date,
                    millis_of_day,
                })
            },
            |s| {
                Self::parse_utc(s)
                    .or_else(|_| Self::parse_utc_millis(s))
                    .ok()
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bound, Date, DateTime, Day, Error, MedlineError, Month, PartialDate, Quarter, Season,
        Seasons, Span, Year,
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
        // Any case since 2026-09-23: PubMed's oldest records shout their
        // months. `January` is still not a `Month` element's spelling; only
        // a `MedlineDate` writes the full name.
        assert_eq!(Month::parse("jan"), Ok(Month::January));
        assert_eq!(Month::parse("JAN"), Ok(Month::January));
        for bad in ["January", "0", "13", "001", "", " 1"] {
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
            PartialDate::parse_medline("2026 Jan-Feb 01"),
            Ok(span(
                b(2026, Some(Month::January), None),
                b(2026, Some(Month::February), Some(1))
            ))
        );
        assert!(PartialDate::parse_medline("2026 Jan-Feb 01 x").is_err());
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
            PartialDate::parse_medline("2022"),
            Ok(PartialDate::Year(y(2022)))
        );
        assert_eq!(
            PartialDate::parse_medline("2017 Feb."),
            Ok(PartialDate::YearMonth(y(2017), Month::February))
        );
        assert_eq!(
            PartialDate::parse_medline("2020 Feb & Mar"),
            Ok(span(
                b(2020, Some(Month::February), None),
                b(2020, Some(Month::March), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("2019 May/June"),
            Ok(span(
                b(2019, Some(Month::May), None),
                b(2019, Some(Month::June), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("2020 May - Jun"),
            Ok(span(
                b(2020, Some(Month::May), None),
                b(2020, Some(Month::June), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("2021 Mar/Apr"),
            Ok(span(
                b(2021, Some(Month::March), None),
                b(2021, Some(Month::April), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("2000 Dec 8-2001 Jan 10"),
            Ok(span(
                b(2000, Some(Month::December), Some(8)),
                b(2001, Some(Month::January), Some(10))
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

    /// The four forms pubmed26n0001, the corpus's oldest file, spells in
    /// upper case; `Jul-AUG` is its own record's mixture.
    #[test]
    fn medline_months_in_any_case() {
        let span = |a: Bound, b: Bound| PartialDate::Span(Span { start: a, end: b });
        let b = |year: u16, month: Option<Month>, day: Option<u8>| Bound {
            year: y(year),
            month,
            day: day.map(d),
        };
        assert_eq!(
            PartialDate::parse_medline("1975 MAR-APR"),
            Ok(span(
                b(1975, Some(Month::March), None),
                b(1975, Some(Month::April), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1976 MAR-APR"),
            Ok(span(
                b(1976, Some(Month::March), None),
                b(1976, Some(Month::April), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1977 JAN-FEB"),
            Ok(span(
                b(1977, Some(Month::January), None),
                b(1977, Some(Month::February), None)
            ))
        );
        assert_eq!(
            PartialDate::parse_medline("1976 Jul-AUG"),
            Ok(span(
                b(1976, Some(Month::July), None),
                b(1976, Some(Month::August), None)
            ))
        );
        // The full names too, which take_month matches before the short
        // ones, and a month element on its own.
        assert_eq!(
            PartialDate::parse_medline("1975 MARCH-APRIL"),
            Ok(span(
                b(1975, Some(Month::March), None),
                b(1975, Some(Month::April), None)
            ))
        );
        assert_eq!(Month::from_name("mar"), Some(Month::March));
    }

    /// Seasonal volumes, the other thing pubmed26n0001 taught us.
    #[test]
    fn medline_seasonal_volumes() {
        let seasons = |from_year: u16, to_year: u16, from: Season, to: Season| {
            Ok(PartialDate::Seasons(Seasons {
                from_year: y(from_year),
                to_year: y(to_year),
                from,
                to,
            }))
        };
        assert_eq!(
            PartialDate::parse_medline("1976-1977 Winter"),
            seasons(1976, 1977, Season::Winter, Season::Winter)
        );
        assert_eq!(
            PartialDate::parse_medline("1977-1978 Fall-Winter"),
            seasons(1977, 1978, Season::Fall, Season::Winter)
        );
        // A season range inside one year, which pubmed26n0002 showed.
        assert_eq!(
            PartialDate::parse_medline("1978 Fall-Winter"),
            seasons(1978, 1978, Season::Fall, Season::Winter)
        );
        assert_eq!(
            PartialDate::parse_medline("2026 Spring-Summer"),
            seasons(2026, 2026, Season::Spring, Season::Summer)
        );
        // The years still stand alone.
        assert_eq!(
            PartialDate::parse_medline("1976-1977"),
            Ok(PartialDate::Span(Span {
                start: Bound {
                    year: y(1976),
                    month: None,
                    day: None
                },
                end: Bound {
                    year: y(1977),
                    month: None,
                    day: None
                }
            }))
        );
        for bad in [
            "1976-1977 Wintr",
            "1976-1977 Winter-",
            "1976-1977 Winter Spring",
        ] {
            assert_eq!(
                PartialDate::parse_medline(bad),
                Err(MedlineError::Unrecognized),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn medline_forms_not_yet_observed_fail() {
        for s in [
            "2026 Jan",
            "2026 Jan..",
            "2026 Febr",
            "2026 Jan-",
            "2026 Jan -Dec",
            "2026 Jan- Dec",
            "2026 Jan / Dec",
            "2026 Jan 1 - 5",
            "2026 Jan-Dec ",
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

    #[test]
    fn millisecond_date_times() {
        let dt = DateTime::parse_utc_millis("2025-07-25T07:35:27.710Z").unwrap();
        assert_eq!(dt.millis(), 710);
        assert_eq!(dt.epoch_seconds(), 1_753_428_927);
        assert_eq!(dt.epoch_millis(), 1_753_428_927_710);
        assert_eq!(dt.to_string(), "2025-07-25T07:35:27.710Z");
        let naive = DateTime::parse_utc_naive("2025-07-25T07:35:27.710513").unwrap();
        assert_eq!(naive, dt, "six fraction digits keep their milliseconds");
        assert_eq!(
            DateTime::parse_utc_naive("2025-07-25T07:35:27.7").unwrap(),
            DateTime::parse_utc_millis("2025-07-25T07:35:27.700Z").unwrap()
        );
        assert_eq!(
            DateTime::parse_utc_naive("2026-01-24T00:00:00").unwrap(),
            DateTime::parse_utc("2026-01-24T00:00:00Z").unwrap()
        );
        for bad in [
            "2026-01-24T00:00:00Z",
            "2026-01-24T00:00:00.",
            "2026-01-24T00:00:00.1234567",
            "2026-01-24T00:00:00.12a",
            "2026-01-24 00:00:00",
        ] {
            assert_eq!(
                DateTime::parse_utc_naive(bad),
                Err(Error::DateTimeNaiveShape),
                "{bad}"
            );
        }
        assert_eq!(
            DateTime::parse_utc_naive("2026-01-24T24:00:00"),
            Err(Error::Time)
        );
        let midnight = DateTime::parse_utc_millis("2020-06-05T00:00:00.000Z").unwrap();
        assert_eq!(
            midnight,
            DateTime::parse_utc("2020-06-05T00:00:00Z").unwrap()
        );
        assert_eq!(midnight.to_string(), "2020-06-05T00:00:00Z");
        assert!(dt > midnight);
        for bad in [
            "2025-07-25T07:35:27Z",
            "2025-07-25T07:35:27.71Z",
            "2025-07-25T07:35:27.7100Z",
            "2025-07-25T07:35:27,710Z",
            "2025-07-25T07:35:27.710",
            "",
        ] {
            assert_eq!(
                DateTime::parse_utc_millis(bad),
                Err(Error::DateTimeMillisShape),
                "{bad}"
            );
        }
        assert_eq!(
            DateTime::parse_utc("2025-07-25T07:35:27.710Z"),
            Err(Error::DateTimeShape)
        );
        assert_eq!(
            DateTime::parse_utc_millis("2025-02-30T00:00:00.000Z"),
            Err(Error::Calendar)
        );
        assert_eq!(size_of::<DateTime>(), 8);
    }

    #[test]
    fn iso_dates() {
        let d = Date::parse_iso("2008-01-01").unwrap();
        assert_eq!(d, Date::from_numbers(2008, 1, 1).unwrap());
        assert_eq!(d.to_string(), "2008-01-01");
        for bad in [
            "2008-1-01",
            "2008-01-1",
            "20080101",
            "2008-01-01T",
            " 2008-01-01",
            "",
        ] {
            assert_eq!(Date::parse_iso(bad), Err(Error::DateShape), "{bad}");
        }
        assert_eq!(Date::parse_iso("2008-02-30"), Err(Error::Calendar));
        assert_eq!(Date::parse_iso("0999-02-03"), Err(Error::YearRange));
    }
}
