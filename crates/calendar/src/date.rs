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
            return year_ordering;
        }

        let month_ordering = self.month.cmp(&other.month);
        if month_ordering.is_ne() {
            return month_ordering;
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
    #[cfg(test)]
    const fn new<const YEAR: u16, const MONTH: u8, const DAY: u8>() -> Date {
        const {
            struct C<const YEAR: u16, const MONTH: u8>();
            impl<const YEAR: u16, const MONTH: u8> C<YEAR, MONTH> {
                const DAY_CAP: u8 = Date::month_day_count(YEAR, MONTH);
            }

            assert!(MONTH > 0);
            assert!(MONTH < 13);
            assert!(DAY > 0);
            assert!(DAY <= C::<YEAR, MONTH>::DAY_CAP);
        };

        Date {
            year: YEAR,
            month: MONTH,
            day: DAY,
        }
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
        Self::add_days(self, 7)
    }

    pub fn subtract_week(&self) -> Date {
        Self::add_days(self, -7)
    }

    pub fn add_days(&self, days: i16) -> Date {
        let total_days = Self::calculate_total_days(self);
        let n: i32 = total_days + days as i32;
        eafs::calculate_gregorian_date(n)
    }

    pub fn try_new(year: u16, month: u8, day: u8) -> Result<Date, InvalidInput> {
        if year < 1 {
            return Err(InvalidInput);
        }

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

    pub fn from_days(days: i32) -> Self {
        assert!(days >= 0);
        eafs::calculate_gregorian_date(days)
    }

    /// Returns the number of the days which have past before the Date.  Given that, 0001-01-01
    pub fn calculate_total_days(&self) -> i32 {
        eafs::calculate_rata_die_from_gregorian_calendar(self)
    }

    pub fn subtract(&self, other: &Date) -> i32 {
        let self_days = self.calculate_total_days();
        let other_days = other.calculate_total_days();
        self_days - other_days
    }
}

mod eafs {
    //! Euclidean affine functions and their application to calendar algorithms DOI:
    //! 10.0002/spe.3172 Neri-Schneider

    use super::Date;
    pub fn calculate_gregorian_date(rata_die: i32) -> Date {
        assert!(rata_die >= 0);
        let n1 = 4 * rata_die + 3;

        let n2 = 3 + 4 * {
            // nc
            n1 % 146097 / 4
        };

        let n3 = 461
            + 5 * {
                // ny
                n2 % 1461 / 4
            };

        let m = n3 / 153;

        let j = if m >= 13 { 1 } else { 0 };
        let year = j + {
            let c = n1 / 146097; //146097 the number of days in 400 years
            // y
            100 * c + {
                // z
                n2 / 1461
            }
        };

        let month = m - 12 * j;
        let day = 1 + {
            // d
            n3 % 153 / 5
        };

        assert!(year >= 0);
        assert!(month >= 0);
        assert!(day >= 0);
        assert!(year <= u16::MAX as i32);
        assert!(month <= u8::MAX as i32);
        assert!(day <= u8::MAX as i32);
        Date::try_new(year as u16, month as u8, day as u8)
            .expect("Failed to calculate a date with Neri-Shneider")
    }

    pub fn calculate_rata_die_from_gregorian_calendar(date: &Date) -> i32 {
        let Date { year, month, day } = date;
        let j = if *month <= 2 { 1 } else { 0 };
        // underflow safety: the structure Date guarantees that year is positive
        let y = *year - j;
        let m = *month as u16 + 12 * j;
        // underflow safety: the structure Date guarantees that day is positive
        let d = day - 1;
        let c = y / 100;

        let ystar = 1461 * (y as i32) / 4 - (c as i32) + (c as i32) / 4;
        // might go negative
        let mstar = (153 * (m as i32) - 457) / 5;
        let ret = ystar + mstar + d as i32;
        assert!(ret >= 0);
        ret
    }

    #[cfg(test)]
    mod test {
        use super::*;
        #[test]
        fn test_roundtrip() {
            for y in 1..=2100 {
                for m in 1..=12 {
                    for d in 1..=31 {
                        if let Ok(input_date) = Date::try_new(y, m, d) {
                            let rata_die = calculate_rata_die_from_gregorian_calendar(&input_date);
                            let date = calculate_gregorian_date(rata_die);
                            assert_eq!(input_date, date);
                        }
                    }
                }
            }
        }

        #[test]
        fn test_invariant() {
            let mut last_rata_die: Option<i32> = None;
            for y in 1..=2100 {
                for m in 1..=12 {
                    for d in 1..=31 {
                        if let Ok(input_date) = Date::try_new(y, m, d) {
                            let rata_die = calculate_rata_die_from_gregorian_calendar(&input_date);
                            if let Some(x) = last_rata_die.replace(rata_die) {
                                assert_eq!(x + 1, rata_die);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod date_add_days {
        use super::*;

        const fn newdate<const YEAR: u16, const MONTH: u8, const DAY: u8>() -> Date {
            Date::new::<YEAR, MONTH, DAY>()
        }

        #[test]
        fn test_regular_year() {
            let inputs = [
                (newdate::<2001, 1, 1>(), newdate::<2001, 1, 8>(), 7),
                (newdate::<2000, 2, 28>(), newdate::<2000, 3, 6>(), 7),
                (newdate::<2001, 2, 28>(), newdate::<2001, 3, 7>(), 7),
            ];

            for (i, (st, expected_date, days)) in inputs.iter().enumerate() {
                let en = st.add_days(*days);
                assert_eq!(en, *expected_date, "case #{} (0-based) failed", i);
            }
        }
    }

    mod date_subtract {
        use super::*;
        #[test]
        fn test_regular_year_duration() {
            let st = Date::new::<2001, 1, 1>();
            let en = Date::new::<2001, 12, 31>();
            let diff = en.subtract(&st);
            assert_eq!(diff, 364);
        }

        #[test]
        fn test_leap_year_duration() {
            let st = Date::new::<2000, 1, 1>();
            let en = Date::new::<2000, 12, 31>();
            let diff = en.subtract(&st);
            assert_eq!(diff, 365);
        }

        #[test]
        fn test_one_day_subtraction() {
            let term1 = Date::new::<2025, 12, 30>();
            let term2 = Date::new::<2025, 12, 29>();
            let diff = term1.subtract(&term2);
            assert_eq!(diff, 1)
        }

        #[test]
        fn test_cross_leap_year() {
            let week_start = Date::new::<2028, 12, 29>();
            let event_start = Date::new::<2029, 1, 4>();
            let diff = event_start.subtract(&week_start);
            assert_eq!(diff, 6)
        }

        #[test]
        fn test_cross_year() {
            let week_start = Date::new::<2025, 12, 29>();
            let event_start = Date::new::<2026, 1, 4>();
            let diff = event_start.subtract(&week_start);
            assert_eq!(diff, 6)
        }
    }
}
