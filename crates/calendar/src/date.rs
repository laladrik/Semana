use core::str::FromStr;
use std::ffi::c_long;
use std::num::ParseIntError;

pub const MINUTES_PER_HOUR: u8 = 60;
pub const MINUTES_PER_DAY: u16 = MINUTES_PER_HOUR as u16 * 24;
pub const SECONDS_PER_DAY: u32 = MINUTES_PER_DAY as u32 * 60;

fn increment_date(date: &Date) -> Date {
    let Date { year, month, day } = date;

    let month_day_count = Date::month_day_count(*year, *month);
    if day + 1 > month_day_count {
        let day = 1;
        let (month, year) = match month + 1 {
            13 => (1u8, year + 1),
            m => (m, *year),
        };
        Date { day, month, year }
    } else {
        Date {
            day: day + 1,
            month: *month,
            year: *year,
        }
    }
}

pub struct DateStream {
    last_date: Date,
}

impl DateStream {
    pub fn new(date: Date) -> Self {
        Self { last_date: date }
    }
}

impl Iterator for DateStream {
    type Item = Date;

    fn next(&mut self) -> Option<Self::Item> {
        let new_date = increment_date(&self.last_date);
        let ret = std::mem::replace(&mut self.last_date, new_date);
        Some(ret)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub struct Date {
    pub year: u16,
    /// 1 .. 12
    pub month: u8,
    /// 1 .. 31
    pub day: u8,
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let year_ordering = self.year.cmp(&other.year);
        if year_ordering.is_ne() {
            return year_ordering
        }

        let month_ordering = self.month.cmp(&other.month);
        if month_ordering.is_ne() {
            return month_ordering
        }

        self.day.cmp(&other.day)
    }
}

impl nanoserde::DeJson for Date {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut std::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        if let nanoserde::DeJsonTok::Str = &mut state.tok {
            let s = core::mem::take(&mut state.strbuf);
            match Date::from_str(&s) {
                Err(_) => Err(state.err_parse("date")),
                Ok(x) => {
                    state.next_tok(input)?;
                    Ok(x)
                }
            }
        } else {
            Err(state.err_token("date"))
        }
    }
}

#[derive(Debug)]
pub enum ParseDateError {
    InvalidInput(InvalidInput),
    ParseIntError(ParseIntError),
    InputIsShort,
}

pub enum ParseTimeError {
    InvalidInput(InvalidInput),
    ParseIntError(ParseIntError),
    UnicodeIsNotSupported,
    InputIsShort,
}

impl FromStr for Time {
    type Err = ParseTimeError;

    // format 23:59
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 5 {
            return Err(ParseTimeError::InputIsShort);
        }

        if !s.is_ascii() {
            return Err(ParseTimeError::UnicodeIsNotSupported);
        }

        let hour = u8::from_str(&s[0..2]).map_err(ParseTimeError::ParseIntError)?;
        let minute = u8::from_str(&s[3..5]).map_err(ParseTimeError::ParseIntError)?;
        Time::try_new(hour, minute).map_err(ParseTimeError::InvalidInput)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone, Copy)]
pub struct Minutes(pub u16);

impl Minutes {
    #[inline]
    pub fn add(self, other: Self) -> Self {
        Minutes(self.0 + other.0)
    }

    #[inline]
    pub fn subtract(self, other: Self) -> Self {
        Minutes(self.0 - other.0)
    }
}

impl Time {
    #[inline]
    pub fn total_minutes(&self) -> Minutes {
        Minutes(self.hour as u16 * MINUTES_PER_HOUR as u16 + self.minute as u16)
    }

    pub fn try_new(hour: u8, minute: u8) -> Result<Time, InvalidInput> {
        if hour > 23 || minute > 59 {
            Err(InvalidInput)
        } else {
            Ok(Time { hour, minute })
        }
    }

    pub const fn midnight() -> Self {
        Self { hour: 0, minute: 0 }
    }

    pub const fn last_minute() -> Self {
        Self {
            hour: 23,
            minute: 59,
        }
    }

    pub fn minutes_from_midnight(&self) -> u16 {
        (self.hour as u16 * MINUTES_PER_HOUR as u16) + self.minute as u16
    }
}

#[derive(Debug, Clone)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
}

impl nanoserde::DeJson for Time {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut std::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        if let nanoserde::DeJsonTok::Str = &mut state.tok {
            let s = core::mem::take(&mut state.strbuf);
            match s.as_str() {
                // the empty string in the time occurs when the event takes the entire; therefore
                // it's assumed the field `all-day` will be "True".
                "" => {
                    state.next_tok(input)?;
                    Ok(Time::midnight())
                }
                non_empty_string => match Time::from_str(non_empty_string) {
                    Ok(x) => {
                        state.next_tok(input)?;
                        Ok(x)
                    }
                    Err(_) => Err(state.err_parse("time")),
                },
            }
        } else {
            Err(state.err_token("time"))
        }
    }
}

#[derive(Debug)]
pub struct InvalidInput;

impl FromStr for Date {
    type Err = ParseDateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // the date has to be like: 2025-10-31
        if s.len() < 10 {
            return Err(ParseDateError::InputIsShort);
        }

        let date_str = &s[0..10];
        let year = u16::from_str(&date_str[0..4]).map_err(ParseDateError::ParseIntError)?;
        let month = u8::from_str(&date_str[5..7]).map_err(ParseDateError::ParseIntError)?;
        let day = u8::from_str(&date_str[8..10]).map_err(ParseDateError::ParseIntError)?;
        Date::try_new(year, month, day).map_err(ParseDateError::InvalidInput)
    }
}

pub struct DateString([u8; 10]);

impl DateString {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("DateString must be built from numbers only and dashes")
    }
}

impl Date {
    /// return the byte representation of the date.
    pub const fn iso_8601(&self) -> DateString {
        DateString([
            (self.year / 1000) as u8 + 48,
            (self.year % 1000 / 100) as u8 + 48,
            (self.year % 100 / 10) as u8 + 48,
            (self.year % 10) as u8 + 48,
            b'-',
            (self.month / 10) + 48,
            (self.month % 10) + 48,
            b'-',
            (self.day / 10) + 48,
            (self.day % 10) + 48,
        ])
    }

    pub fn add_week(&self) -> Date {
        add_days(self, 7)
    }

    pub fn subtract_week(&self) -> Date {
        add_days(self, -7)
    }

    pub fn add_days(&self, days: i16) -> Date {
        add_days(self, days)
    }

    pub const fn month_day_count(year: u16, month: u8) -> u8 {
        match month {
            2 => {
                if Self::is_leap_year(year) {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    pub fn try_new(year: u16, month: u8, day: u8) -> Result<Date, InvalidInput> {
        if !(month > 0 && month <= 12) {
            return Err(InvalidInput);
        }

        let day_max = Date::month_day_count(year, month);
        if !(day > 0 && day <= day_max) {
            return Err(InvalidInput);
        }

        Ok(Date { year, month, day })
    }

    #[inline(always)]
    pub const fn is_leap_year(year: u16) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    pub fn days_from_epoch(&self) -> i32 {
        // Days from months (approximate).  the 31 from December is skipped, because when we pass
        // December we pass the year.  Given that the days are in `year_days` already.
        let month_capacities = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30];
        let month_days: i32 = month_capacities.iter().take(self.month as usize - 1).sum();

        let year_days = years_to_days(self.year);
        let total_days = self.day as i32 + month_days + year_days;

        // Adjust for leap years in current year
        if self.month > 2 && Self::is_leap_year(self.year) {
            total_days + 1
        } else {
            total_days
        }
    }

    pub fn subtract(&self, other: &Date) -> i32 {
        let self_days = self.days_from_epoch();
        let other_days = other.days_from_epoch();
        self_days - other_days
    }
}

fn years_to_days(year: u16) -> i32 {
    const START: i32 = 1970;
    let years_since_the_start: i32 = (year as i32) - START;
    let leap_years = years_since_the_start + 2 - 1;
    years_since_the_start
        * 365
        + leap_years / 4
        - (leap_years / 100)
        + leap_years / 400
}

use std::ffi::c_char;
use std::ffi::c_int;

#[allow(non_camel_case_types)]
#[repr(C)]
struct c_tm {
    /// Seconds          [0, 60]
    tm_sec: c_int,
    /// Minutes          [0, 59]
    tm_min: c_int,
    /// Hour             [0, 23]
    tm_hour: c_int,
    /// Day of the month [1, 31]
    tm_mday: c_int,
    /// Month            [0, 11]  (January = 0)
    tm_mon: c_int,
    /// Year minus 1900
    tm_year: c_int,
    /// Day of the week  [0, 6]   (Sunday = 0)
    tm_wday: c_int,
    /// Day of the year  [0, 365] (Jan/01 = 0)
    tm_yday: c_int,
    /// Daylight savings flag
    tm_isdst: c_int,
    /// Seconds East of UTC
    tm_gmtoff: c_long,
    /// Timezone abbreviation
    tm_zone: *mut c_char,
}

const TM_YEAR_SHIFT: i16 = -1900;
const TM_MONTH_SHIFT: i16 = -1;

#[allow(non_camel_case_types)]
type c_time_t = u64;

#[link(name = "c")]
unsafe extern "C" {
    /// out is nullable
    fn time(out: *mut c_time_t) -> c_time_t;
    fn localtime(time: *const c_time_t) -> *mut c_tm;
    fn localtime_r(time: *const c_time_t, result: *mut c_tm) -> *mut c_tm;
    /// c_tm::tm_yday and c_tm::tm_wday are ignored.  Reference: ctime(3)
    fn mktime(broken_time: *const c_tm) -> c_time_t;
}

fn add_days(from: &Date, days: i16) -> Date {
    // SAFETY: localtime can't fail with the current time.  Reference: ctime(3)
    unsafe {
        let now_seconds: c_time_t = time(std::ptr::null_mut());
        let mut now_broken: c_tm = std::mem::zeroed();
        let ret: *const _ = localtime_r(&now_seconds, &mut now_broken);
        if ret.is_null() {
            panic!("we can't get the today's date");
        }

        now_broken.tm_year = (from.year as i32 + TM_YEAR_SHIFT as i32) as _;
        now_broken.tm_mon = (from.month as i32 + TM_MONTH_SHIFT as i32) as _;
        now_broken.tm_mday = from.day as _;
        let from_time_seconds: c_time_t = mktime(&now_broken as _);

        let diff = days as i64 * SECONDS_PER_DAY as i64;
        let result_seconds: c_time_t = if diff > 0 {
            from_time_seconds + diff as u64
        } else {
            from_time_seconds - diff.abs() as u64
        };

        let result_broken: *const c_tm = localtime(&result_seconds as _);
        let year = (*result_broken).tm_year as i32 - TM_YEAR_SHIFT as i32;
        assert!(year > 0 && year <= u16::MAX as i32);
        let month = (*result_broken).tm_mon - TM_MONTH_SHIFT as i32;
        assert!(month > 0 && month < u8::MAX as i32);

        let ret = Date {
            year: year as u16,
            month: month as u8,
            day: (*result_broken).tm_mday as _,
        };

        let ret_days = ret.subtract(from);
        assert_eq!(ret_days, days.into(), "the result date is wrong: {:?}", ret);
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod years_to_days {
        use super::*;
        #[test]
        fn test_the_first_leap_year() {
            let days = years_to_days(1972);
            assert_eq!(days, 365 * 2);
        }

        #[test]
        fn test_the_year_after_the_first_leap_year() {
            let days = years_to_days(1973);
            assert_eq!(days, 365 * 3 + 1);
        }
    }

    mod date_subtract {
        use super::*;
        #[test]
        fn test_one_day_subtraction() {
            let term1 = Date {
                year: 2025,
                month: 12,
                day: 30,
            };
            let term2 = Date {
                year: 2025,
                month: 12,
                day: 29,
            };

            let diff = term1.subtract(&term2);
            assert_eq!(diff, 1)
        }

        #[test]
        fn test_cross_leap_year() {
            let week_start = Date {
                year: 2028,
                month: 12,
                day: 29,
            };
            let event_start = Date {
                year: 2029,
                month: 1,
                day: 4,
            };
            let diff = event_start.subtract(&week_start);
            assert_eq!(diff, 6)
        }

        #[test]
        fn test_cross_year() {
            let week_start = Date {
                year: 2025,
                month: 12,
                day: 29,
            };
            let event_start = Date {
                year: 2026,
                month: 1,
                day: 4,
            };
            let diff = event_start.subtract(&week_start);
            assert_eq!(diff, 6)
        }
    }
}
