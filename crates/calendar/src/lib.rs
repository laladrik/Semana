pub mod obtain;
pub mod render;
pub mod ui;
pub mod types;
pub mod date;

use nanoserde::DeJson;
#[derive(Debug)]
pub enum Error<'s> {
    InvalidDate(&'s str),
    InvalidTime(&'s str),
}

#[derive(DeJson, Debug)]
pub struct Event {
    title: String,
    #[nserde(rename = "start-date")]
    start_date: date::Date,
    #[nserde(rename = "start-time")]
    start_time: date::Time,
    #[nserde(rename = "end-date")]
    end_date: date::Date,
    #[nserde(rename = "end-time")]
    end_time: date::Time,
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

pub trait TextCreate {
    type Result;
    fn text_create(&self, s: &str) -> Self::Result;
}

pub struct EventRange {
    pub start_date: date::Date,
    pub start_time: date::Time,
    pub end_date: date::Date,
    pub end_time: date::Time,
    pub calendar_color: Color,
}

pub struct EventData {
    pub event_ranges: Vec<EventRange>,
    pub titles: Vec<String>,
    pub lanes: Vec<(Lane, Lane)>,
}

pub type Lane = u8;

#[cfg(test)]
mod tests {
    use crate::date::{Date, Time};
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
