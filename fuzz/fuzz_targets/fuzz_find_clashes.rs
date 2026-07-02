#![no_main]

use arbitrary::{self, Unstructured};
use calendar::{Color, Event, date};
use libfuzzer_sys::fuzz_target;

struct Week {
    start: date::Date,
    events: Vec<Event>,
}

impl core::fmt::Debug for Week {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Week").field("start", &self.start).finish()
    }
}

impl<'a> arbitrary::Arbitrary<'a> for Week {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let count = u8::arbitrary(u)?;
        let mut events = Vec::with_capacity(count as usize);
        let start = date::Date {
            year: u16::arbitrary(u)?,
            month: u8::arbitrary(u)?,
            day: u8::arbitrary(u)?,
        };

        for _ in 0..count {
            let start_date = date::Date {
                year: start.year,
                month: start.month,
                day: start.day + (u8::arbitrary(u)? % 7),
            };

            let end_date = start_date.clone();
            let start_time = date::Time {
                hour: u8::arbitrary(u)?,
                minute: u8::arbitrary(u)?,
            };

            let end_time = date::Time {
                hour: start_time.hour + (u8::arbitrary(u)? % (24 - start_time.hour)),
                minute: start_time.minute + (u8::arbitrary(u)? % (60 - start_time.minute)),
            };

            let event = Event {
                description: u32::arbitrary(u)?,
                title: String::arbitrary(u)?,
                start_date,
                start_time,
                end_date,
                end_time,

                calendar_color: Color(u32::arbitrary(u)?),
            };

            events.push(event);
        }
        events.sort_by(|lt, rt| lt.start_date.cmp(&rt.start_date));

        Ok(Self { start, events })
    }
}

fn fuzzme(input: Week) {
    calendar::obtain::find_clashes(
        &input.events,
        &input.start,
        calendar::obtain::short_event_clash_condition,
    );
}

fuzz_target!(|input: Week| {
    fuzzme(input);
});
