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

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
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
    InputIsShort,
}

impl FromStr for Time {
    type Err = ParseTimeError;

    // format 23:59
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 5 {
            return Err(ParseTimeError::InputIsShort);
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
        let mut total_days = 0;

        const START: i32 = 1970;
        let years_since_the_start: i32 = (self.year as i32) - START;
        // Days from years
        total_days += years_since_the_start * 365;
        total_days += years_since_the_start / 4;
        total_days -= years_since_the_start / 100;
        total_days += years_since_the_start / 400;

        // Days from months (approximate)
        let month_days = [1, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for m in 1..(self.month as usize) {
            total_days += month_days[m - 1];
        }

        // Add days
        total_days += self.day as i32;

        // Adjust for leap years in current year
        if self.month > 2 && Self::is_leap_year(self.year) {
            total_days += 1;
        }

        total_days
    }

    pub fn subtract(&self, other: &Date) -> i32 {
        let self_days = self.days_from_epoch();
        let other_days = other.days_from_epoch();
        self_days - other_days
    }
}

use std::ffi::c_char;
use std::ffi::c_int;

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

type c_time_t = u64;
unsafe extern "C" {
    /// out is nullable
    fn time(out: *mut c_time_t) -> c_time_t;
    fn localtime(time: *const c_time_t) -> *mut c_tm;
    /// c_tm::tm_yday and c_tm::tm_wday are ignored.  Reference: ctime(3)
    fn mktime(broken_time: *const c_tm) -> c_time_t;
}

fn add_days(from: &Date, days: i16) -> Date {
    // SAFETY: localtime can't fail with the current time.  Reference: ctime(3)
    unsafe {
        let now_seconds: c_time_t = time(std::ptr::null_mut());
        let now_broken: *mut c_tm = localtime(&now_seconds as _);
        if now_broken.is_null() {
            panic!("we can't get the today's date");
        }

        (*now_broken).tm_year = from.year as _;
        (*now_broken).tm_mon = from.month as _;
        (*now_broken).tm_mday = from.day as _;
        let from_time_seconds: c_time_t = mktime(now_broken);
        let result_seconds: c_time_t = if days > 0 {
            from_time_seconds + (days as u64 * SECONDS_PER_DAY as u64)
        } else {
            from_time_seconds - (days as u64 * SECONDS_PER_DAY as u64)
        };

        let result_broken: *const c_tm = localtime(&result_seconds as _);
        Date {
            year: (*result_broken).tm_year as _,
            month: (*result_broken).tm_mon as _,
            day: (*result_broken).tm_mday as _,
        }
    }
}
