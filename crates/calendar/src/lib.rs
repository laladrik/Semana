#![no_std]
#![cfg_attr(not(test), no_main)]

pub mod date;
pub mod obtain;
pub mod render;
pub mod types;
pub mod ui;
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use nanoserde::DeJson;
#[derive(Debug)]
pub enum Error<'s> {
    InvalidDate(&'s str),
    InvalidTime(&'s str),
}

#[derive(DeJson)]
pub struct JsonInputEvent {
    description: String,
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
    calendar_color: Option<Color>,
}

pub struct Event {
    description: u32,
    // FIXME(alex): store the string in a separated data storage
    title: String,
    start_date: date::Date,
    start_time: date::Time,
    end_date: date::Date,
    end_time: date::Time,
    calendar_color: Color,
}

#[derive(Clone, Copy)]
pub struct ColorDiff(pub [f32; 3]);

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Color(pub u32);

impl Color {
    const BLACK: Color = Color(0x000000ff);

    pub fn adjust(&self, diff: &ColorDiff) -> Color {
        // NOTE(alex): may add with overflow?
        const RED_SHIFT: u32 = 24;
        const GREEN_SHIFT: u32 = 16;
        const BLUE_SHIFT: u32 = 8;
        const _ALPHA_SHIFT: u32 = 0;

        let r = (self.0 & (0xff << RED_SHIFT)) >> RED_SHIFT;
        let g = (self.0 & (0xff << GREEN_SHIFT)) >> GREEN_SHIFT;
        let b = (self.0 & (0xff << BLUE_SHIFT)) >> BLUE_SHIFT;
        // NOTE(alex): converting -0.5f32 to u32 results 0u32
        let new_r: u32 = (r as i32 + ((diff.0[0] * 255f32) as i32)) as u32;
        let new_g: u32 = (g as i32 + ((diff.0[1] * 255f32) as i32)) as u32;
        let new_b: u32 = (b as i32 + ((diff.0[2] * 255f32) as i32)) as u32;

        let val = Color::BLACK.0 | new_r << RED_SHIFT | new_g << GREEN_SHIFT | new_b << BLUE_SHIFT;
        Color(val)
    }
}

impl From<Color> for u32 {
    fn from(val: Color) -> Self {
        val.0
    }
}

impl nanoserde::DeJson for Color {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut core::str::Chars,
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

pub struct EventRange {
    pub start_date: date::Date,
    pub start_time: date::Time,
    pub end_date: date::Date,
    pub end_time: date::Time,
}

pub struct EventTable {
    pub calendar_colors: Vec<Color>,
    pub event_ranges: Vec<EventRange>,
    pub titles: Vec<String>,
    pub description_handles: Vec<u32>,
    pub description_strings: Vec<String>,
    pub lanes: Vec<(Lane, Lane)>,
}

impl EventTable {
    pub fn obtain_description(&self, event: u32) -> Option<&str> {
        self.description_handles
            .get(event as usize)
            .and_then(|handle: &u32| self.description_strings.get(*handle as usize))
            .map(String::as_str)
    }

    pub fn obtain_title(&self, event: u32) -> Option<&str> {
        self.titles.get(event as usize).map(|s| s.as_str())
    }

    pub fn obtain_range(&self, event: u32) -> Option<&EventRange> {
        self.event_ranges.get(event as usize)
    }
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
