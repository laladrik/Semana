pub mod obtain;
pub mod render;
pub mod ui;

use core::str::FromStr;
use std::num::ParseIntError;

use nanoserde::DeJson;
#[derive(Debug)]
pub enum Error<'s> {
    InvalidDate(&'s str),
    InvalidTime(&'s str),
}

#[derive(DeJson, Debug)]
struct Event {
    title: String,
    #[nserde(rename = "start-date")]
    start_date: Date,
    #[nserde(rename = "start-time")]
    start_time: Time,
    #[nserde(rename = "end-date")]
    end_date: Date,
    #[nserde(rename = "end-time")]
    end_time: Time,
    #[nserde(rename = "all-day")]
    all_day: String,
    #[nserde(rename = "calendar-color")]
    calendar_color: Color,
}

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Color(u32);

#[cfg(test)]
impl Color {
    const BLACK: Color = Color(0x000000ff);
}

impl From<Color> for u32 {
    fn from(val: Color) -> Self {
        val.0
    }
}

impl std::fmt::Debug for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Color")
            .field(&format_args!("#{:#x}", self.0))
            .finish()
    }
}

impl nanoserde::DeJson for Color {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut std::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        if let nanoserde::DeJsonTok::Str = &mut state.tok {
            let s = core::mem::take(&mut state.strbuf);
            let s_without_sharp = &s[1..];
            match u32::from_str_radix(s_without_sharp, 16) {
                Err(_) => Err(state.err_parse("Color")),
                Ok(x) => {
                    state.next_tok(input)?;
                    Ok(Color(x))
                }
            }
        } else {
            Err(state.err_token("Color"))
        }
    }
}

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

pub trait TextCreate {
    type Result;
    fn text_create(&self, s: &str) -> Self::Result;
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

struct DateString([u8; 10]);

impl DateString {
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("DateString must be built from numbers only and dashes")
    }
}

impl Date {
    /// return the byte representation of the date.
    const fn iso_8601(&self) -> DateString {
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

    const fn month_day_count(year: u16, month: u8) -> u8 {
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

    fn try_new(year: u16, month: u8, day: u8) -> Result<Date, InvalidInput> {
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
    const fn is_leap_year(year: u16) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    fn days_from_epoch(&self) -> i32 {
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

    fn subtract(&self, other: &Date) -> i32 {
        let self_days = self.days_from_epoch();
        let other_days = other.days_from_epoch();
        self_days - other_days
    }
}

pub struct EventRange {
    pub start_date: Date,
    pub start_time: Time,
    pub end_date: Date,
    pub end_time: Time,
    pub calendar_color: Color,
}

pub struct EventData {
    pub event_ranges: Vec<EventRange>,
    pub titles: Vec<String>,
    pub lanes: Vec<(Lane, Lane)>,
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

const MINUTES_PER_HOUR: u8 = 60;
const MINUTES_PER_DAY: u16 = MINUTES_PER_HOUR as u16 * 24;

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
pub struct Minutes(u16);

impl Minutes {
    #[inline]
    fn add(self, other: Self) -> Self {
        Minutes(self.0 + other.0)
    }

    #[inline]
    fn subtract(self, other: Self) -> Self {
        Minutes(self.0 - other.0)
    }
}

impl Time {
    #[inline]
    fn total_minutes(&self) -> Minutes {
        Minutes(self.hour as u16 * MINUTES_PER_HOUR as u16 + self.minute as u16)
    }

    fn try_new(hour: u8, minute: u8) -> Result<Time, InvalidInput> {
        if hour > 23 || minute > 59 {
            Err(InvalidInput)
        } else {
            Ok(Time { hour, minute })
        }
    }

    const fn midnight() -> Self {
        Self { hour: 0, minute: 0 }
    }

    const fn last_minute() -> Self {
        Self {
            hour: 23,
            minute: 59,
        }
    }

    fn minutes_from_midnight(&self) -> u16 {
        (self.hour as u16 * MINUTES_PER_HOUR as u16) + self.minute as u16
    }
}

pub type Lane = u8;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_data_dejson() {
        #[derive(nanoserde::DeJson)]
        struct Item {
            date: Date,
            time: Time,
        }

        let input = "{\"date\": \"2025-10-27\", \"time\": \"23:58\" }";
        let output: Result<Item, _> = nanoserde::DeJson::deserialize_json(input);
        let ret = output.unwrap();
        let date = ret.date;
        let time = ret.time;
        assert_eq!(date.year, 2025);
        assert_eq!(date.month, 10);
        assert_eq!(time.hour, 23);
        assert_eq!(time.minute, 58);
    }
}
