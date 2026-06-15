use crate::{Color, EventRange};
use alloc::string::String;
use alloc::vec::Vec;

use super::date::{Date, DateStream, MINUTES_PER_DAY, Minutes, Time};
use super::{Event, EventTable, JsonInputEvent};
pub trait JsonParser {
    type Error;

    fn parse<'data, 'me: 'data>(
        &'me self,
        bytes: &'data str,
    ) -> Result<Vec<JsonInputEvent>, Self::Error>;
}

pub struct NanoSerde;
impl JsonParser for NanoSerde {
    type Error = nanoserde::DeJsonErr;

    fn parse<'data, 'me: 'data>(
        &'me self,
        bytes: &'data str,
    ) -> Result<Vec<JsonInputEvent>, Self::Error> {
        nanoserde::DeJson::deserialize_json(bytes)
    }
}

pub type EventVec = Vec<Event>;

#[derive(Debug)]
pub enum Error<PE> {
    InvalidUnicode(core::str::Utf8Error),
    Parse(PE),
    DurationIsTooBig,
}

pub mod khal {
    use super::Date;
    use super::ObtainArguments;
    pub fn week_arguments(from: &Date) -> ObtainArguments<'_> {
        ObtainArguments {
            from,
            duration_days: 7,
            backend_bin_path: "khal",
        }
    }
}

pub struct ObtainArguments<'s> {
    // date in the format YYYY-MM-DD
    pub from: &'s Date,
    // date in the format YYYY-MM-DD
    pub duration_days: u8,
    // path to khal
    pub backend_bin_path: &'s str,
}

#[derive(Default)]
struct Clash {
    event_ends: Vec<Minutes>,
    lanes: Vec<Lane>,
    end: Minutes,
}

impl Clash {
    fn flush(&mut self, into: &mut impl Extend<(Lane, Lane)>, lane_count: Lane) {
        self.event_ends.clear();
        let iter = self.lanes.drain(..).map(|lane| (lane, lane_count));
        into.extend(iter);
        self.end = Minutes::default();
    }

    fn push(&mut self, event_end: Minutes, lane: Lane) {
        self.end = self.end.max(event_end);
        self.event_ends.push(event_end);
        self.lanes.push(lane);
    }
}

type Lane = u8;

// return (n, None) -> new lane has to be created
// return (n, Some(x)) -> stays in the lane n
fn find_free_lane(new_event_begin_minutes: Minutes, clash: &Clash) -> Option<Lane> {
    let lane_index: Option<usize> = clash
        .event_ends
        .iter()
        .enumerate()
        .filter(|(_, end)| **end <= new_event_begin_minutes)
        .fold(None, |acc, item| {
            let (lane_index, end): (usize, &Minutes) = item;
            match acc {
                None => Some((lane_index, end)),
                Some((acc_lane_index, acc_end)) => {
                    // it's guaranteed that `acc_end` and `end` are not bigger than
                    // new_event_begin_minutes. `acc_end` is obtainend from the `end` which is the
                    // closest one to new_event_begin_minutes by this moment.  `end` can't be
                    // bigger, because all the they are filtered out;
                    let acc_diff = new_event_begin_minutes.subtract(*acc_end);
                    let diff = new_event_begin_minutes.subtract(*end);
                    if diff <= acc_diff {
                        Some((lane_index, end))
                    } else {
                        Some((acc_lane_index, end))
                    }
                }
            }
        })
        .map(|(lane_index, _acc_end)| lane_index);

    lane_index.map(|i| unsafe { *clash.lanes.get_unchecked(i) })
}

fn add_description(item: String, storage: &mut Vec<String>) -> u32 {
    assert!(storage.len() < u32::MAX as usize);
    if storage.last().filter(|last| last == &&item).is_none() {
        storage.push(item);
    }

    storage.len() as u32 - 1
}

pub fn parse_events<OutputParser>(
    json_parser: &OutputParser,
    bytes: &str,
    date: &Date,
    default_calendar_color: Color,
) -> Result<Events, Error<OutputParser::Error>>
where
    OutputParser: JsonParser,
{
    let mut week_schedule = Events {
        short: Vec::new(),
        long: Vec::new(),
        long_event_descriptions: Vec::new(),
        short_event_descriptions: Vec::new(),
    };

    let date_stream = DateStream::new(date.clone()).take(7);

    let agendas = bytes
        .split('\n')
        .take(7)
        .take_while(|p| !p.is_empty())
        .zip(date_stream);

    let last_day_in_the_range: Date = date.add_days(6);
    for item in agendas {
        let (agenda_json, date): (&str, Date) = item;
        let agenda: Vec<JsonInputEvent> = json_parser.parse(agenda_json).map_err(Error::Parse)?;
        let event_items = agenda
            .into_iter()
            .filter_map(|event: JsonInputEvent| short_event_filter(event, &date));

        for item in event_items {
            let (is_short, mut json_event): (bool, JsonInputEvent) = item;
            // The end date of event is shortened down to the last day of the week for the case
            // when a long event DOES NOT end by the end of the current week.
            json_event.end_date = json_event.end_date.min(last_day_in_the_range.clone());

            let description = core::mem::take(&mut json_event.description);
            let description_handle: u32 = if is_short {
                add_description(description, &mut week_schedule.short_event_descriptions)
            } else {
                add_description(description, &mut week_schedule.long_event_descriptions)
            };

            let event = Event {
                description: description_handle,
                title: json_event.title,
                start_date: json_event.start_date,
                start_time: json_event.start_time,
                end_date: json_event.end_date,
                end_time: json_event.end_time,
                calendar_color: json_event.calendar_color.unwrap_or(default_calendar_color),
            };

            if is_short {
                week_schedule.short.push(event)
            } else {
                week_schedule.long.push(event)
            }
        }
    }

    Ok(week_schedule)
}

impl EventTable {
    pub fn calculate_biggest_clash(&self) -> Lane {
        self.lanes
            .iter()
            .map(|(_, total_lane_count)| *total_lane_count)
            .max()
            .unwrap_or(0)
    }
}

pub struct Events {
    /// The array of events which span across multiple days.
    long: EventVec,
    /// The array of events which are within a day.
    short: EventVec,
    long_event_descriptions: Vec<String>,
    short_event_descriptions: Vec<String>,
}

pub struct WeekScheduleWithLanes {
    pub long: EventTable,
    pub short: EventTable,
}

impl WeekScheduleWithLanes {
    pub fn long_events_titles(&self) -> impl Iterator<Item = &str> {
        self.long.titles.iter().map(String::as_str)
    }

    pub fn short_events_titles(&self) -> impl Iterator<Item = &str> {
        self.short.titles.iter().map(String::as_str)
    }
}

pub fn get_lanes(events: Events, start_date: &Date) -> WeekScheduleWithLanes {
    let long_lanes: Vec<(Lane, Lane)> =
        find_clashes(&events.long, start_date, long_event_clash_condition);

    let short_lanes: Vec<(Lane, Lane)> =
        find_clashes(&events.short, start_date, short_event_clash_condition);

    let create = |event: Event| -> (EventRange, String, u32, Color) {
        let Event {
            description,
            title,
            start_date,
            start_time,
            end_date,
            end_time,
            calendar_color,
        } = event;
        let range = EventRange {
            start_date,
            start_time,
            end_date,
            end_time,
        };
        (range, title, description, calendar_color)
    };

    let n = events.long.len();
    let mut long_event_ranges: Vec<EventRange> = Vec::with_capacity(n);
    let mut long_event_titles: Vec<String> = Vec::with_capacity(n);
    let mut long_descriptions: Vec<u32> = Vec::with_capacity(n);
    let mut long_calendar_colors: Vec<Color> = Vec::with_capacity(n);
    for long_event in events.long.into_iter() {
        let (range, title, description, calendar_color) = create(long_event);
        long_event_ranges.push(range);
        long_event_titles.push(title);
        long_descriptions.push(description);
        long_calendar_colors.push(calendar_color);
    }

    let n = events.short.len();
    let mut short_event_ranges: Vec<EventRange> = Vec::with_capacity(n);
    let mut short_event_titles: Vec<String> = Vec::with_capacity(n);
    let mut short_descriptions: Vec<u32> = Vec::with_capacity(n);
    let mut short_calendar_colors: Vec<Color> = Vec::with_capacity(n);
    for short_event in events.short.into_iter() {
        let (range, title, description, calendar_color) = create(short_event);
        short_event_ranges.push(range);
        short_event_titles.push(title);
        short_descriptions.push(description);
        short_calendar_colors.push(calendar_color);
    }

    WeekScheduleWithLanes {
        long: EventTable {
            event_ranges: long_event_ranges,
            titles: long_event_titles,
            lanes: long_lanes,
            description_handles: long_descriptions,
            description_strings: events.long_event_descriptions,
            calendar_colors: long_calendar_colors,
        },

        short: EventTable {
            event_ranges: short_event_ranges,
            titles: short_event_titles,
            lanes: short_lanes,
            description_handles: short_descriptions,
            description_strings: events.short_event_descriptions,
            calendar_colors: short_calendar_colors,
        },
    }
}

type ClashCondition = fn(is_new_day: bool, event_end: Minutes, clash_end: Minutes) -> bool;

fn short_event_clash_condition(is_new_day: bool, event_start: Minutes, clash_end: Minutes) -> bool {
    event_start < clash_end && !is_new_day
}

fn long_event_clash_condition(_is_new_day: bool, event_start: Minutes, clash_end: Minutes) -> bool {
    event_start < clash_end
}

fn find_clashes(
    events: &[Event],
    start_date: &Date,
    condition: ClashCondition,
) -> Vec<(Lane, Lane)> {
    let mut last_clash = Clash::default();
    let mut current_date: &Date = start_date;
    let mut lane_count = 0;
    let mut ret: Vec<(Lane, Lane)> = Vec::new();
    for event in events {
        // the difference between the first day of the week (start_day) and the start day of the
        // event.
        let start_day_diff: i32 = event.start_date.subtract(start_date);
        // the difference between the first day of the week (start_day) and the end day of the
        // event.
        let end_day_diff: i32 = event.end_date.subtract(start_date);
        assert!(
            (0..7).contains(&start_day_diff),
            "the events must start within the week"
        );
        assert!(
            (0..7).contains(&end_day_diff),
            "the events must finish within the week"
        );
        let start_date_days: Minutes = Minutes(start_day_diff as u16 * MINUTES_PER_DAY);
        let total_event_start: Minutes = event.start_time.total_minutes().add(start_date_days);
        //let event_start: Minutes = event.start_time.total_minutes().add(days);
        let (rect_lane, new_lane_count, does_replace): (Lane, Lane, bool) = {
            let clash: &Clash = &last_clash;
            let is_new_day = &event.start_date != current_date;
            if is_new_day {
                current_date = &event.start_date;
            }

            let has_collision = condition(is_new_day, total_event_start, clash.end);
            if has_collision {
                let free_lane = find_free_lane(total_event_start, clash);
                match free_lane {
                    // All lanes are busy, creating new one.
                    None => (lane_count, lane_count + 1, !has_collision),
                    Some(lane) => (lane, lane_count, !has_collision),
                }
            } else {
                (0, 1, !has_collision)
            }
        };

        if does_replace {
            last_clash.flush(&mut ret, lane_count);
        }

        lane_count = new_lane_count;
        let end_date_days: Minutes = Minutes(end_day_diff as u16 * MINUTES_PER_DAY);
        last_clash.push(event.end_time.total_minutes().add(end_date_days), rect_lane);
    }

    last_clash.flush(&mut ret, lane_count);
    ret
}

/// Given the 3 types of events:
/// 1. Long event: spans across multiple days (greater than or equal 24h).
/// 2. Short event: stays within a day. (less than 24h)
/// 3. CrossNight event: a short event but which start one day and finishes on the following (less than 24h).
///
/// We canonicalize them into 2 types (long and short).  The CrossNight event is turned into a
/// short event cropping its head or tail. The head is cropped if the starting date of `event`
/// equals to `date`.  The tail is cropped if the ending date of `event` equal to `date`.  This
/// algorithm is based on the _assumption_ that the function `short_event_filter` is called for an
/// event of this kind _twice_.
fn short_event_filter(mut event: JsonInputEvent, date: &Date) -> Option<(bool, JsonInputEvent)> {
    let is_all_day: bool = match event.all_day.as_str() {
        "True" => true,
        "False" => false,
        x => panic!("unexpected string in the field \"all-day\": {:?}", x),
    };

    if is_all_day {
        event.start_time = Time::midnight();
        event.end_time = Time::last_minute();
    }

    let event_type: EventType = determine_event_type(&event, is_all_day);
    match event_type {
        EventType::Short => Some((true, event)),
        EventType::Long => {
            if event.start_date == *date {
                Some((false, event))
            } else {
                None
            }
        }
        EventType::CrossNight => {
            let cropped_event: JsonInputEvent = crop_event(date, event);
            Some((true, cropped_event))
        }
    }
}

enum EventType {
    // event shorter than 24 hours
    Short,
    // event longer than 24 hours
    Long,
    // event shorter than 24 hours, but finishes on the following day
    CrossNight,
}

/// An event which finishes on the following day is split into two events. The event occurs for
/// every day it lasts for.  In every occurence it has the same value in [`Event::start_date`] and
/// [`Event::end_date`].  Given that, there are two occurences of the same event with the same
/// properties.  One for the day where it starts, one for the day where it finishes.  The goal:
/// split the event into two events to alleviate the render of the event.  Given that and that
/// there are two occurrences, the problem converts from splitting to cropping each occurrence of
/// the event.
///
/// The first occurence is turned into the event which lasts until the last minute of its day, the
/// second occurence of the event lasts until the end, and it starts from the midnight of the
/// following day.
///
/// The function crops the event and return its cropped version.  If `date` matches
/// [`Event::start_date`], it crops the second "half" of the event. If `date` matches
/// [`Event::end_date`], it crops the first "half".
///
/// It assumes that it's called only for the events which start on one day and finishes on the
/// following day.
fn crop_event(date: &Date, event: JsonInputEvent) -> JsonInputEvent {
    if date == &event.start_date {
        JsonInputEvent {
            description: event.description,
            start_date: event.start_date.clone(),
            start_time: event.start_time,
            end_date: event.start_date,
            end_time: Time::last_minute(),
            title: event.title,
            all_day: event.all_day,
            calendar_color: event.calendar_color,
        }
    } else if date == &event.end_date {
        JsonInputEvent {
            start_date: event.end_date.clone(),
            start_time: Time::midnight(),
            end_date: event.end_date,
            end_time: event.end_time,
            title: event.title,
            description: event.description,
            all_day: event.all_day,
            calendar_color: event.calendar_color,
        }
    } else {
        panic!("only an event which shorter than 24 hours can be cropped")
    }
}

fn determine_event_type(event: &JsonInputEvent, is_all_day: bool) -> EventType {
    let sd: &Date = &event.start_date;
    let ed: &Date = &event.end_date;
    let st: &Time = &event.start_time;
    let et: &Time = &event.end_time;
    let event_duration_days: i32 = ed.subtract(sd);
    assert!(event_duration_days >= 0);
    match event_duration_days {
        0 if is_all_day => EventType::Long,
        0 => EventType::Short,
        1 => {
            const FULL_DAY: u16 = 24 * 60;
            let event_duration_to_midnight: u16 = FULL_DAY - st.hour as u16 * 60 - st.minute as u16;
            let event_duration_after_midnight: u16 = et.hour as u16 * 60u16 + et.minute as u16;
            let event_duration: u16 = event_duration_to_midnight + event_duration_after_midnight;
            if event_duration >= FULL_DAY {
                EventType::Long
            } else {
                EventType::CrossNight
            }
        }
        _ => EventType::Long,
    }
}

pub struct WeekData {
    pub agenda: WeekScheduleWithLanes,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::ToOwned;
    use core::str::FromStr;
    #[track_caller]
    fn create_date(s: &str) -> Date {
        match Date::from_str(s) {
            Ok(x) => x,
            Err(_) => panic!("can't create Date from {}", s),
        }
    }

    #[track_caller]
    fn create_time(s: &str) -> Time {
        match Time::from_str(s) {
            Ok(x) => x,
            Err(_) => panic!("can't create Time from {}", s),
        }
    }

    #[test]
    fn test_short_event_clash() {
        let create_event = |title: &str, start_time: &str, end_time: &str| JsonInputEvent {
            description: String::default(),
            calendar_color: crate::Color::BLACK,
            title: title.to_owned(),
            start_date: create_date("2025-11-03"),
            start_time: create_time(start_time),
            end_date: create_date("2025-11-03"),
            end_time: create_time(end_time),
            all_day: "False".to_owned(),
        };

        let events: Vec<JsonInputEvent> = Vec::from_iter([
            create_event("first", "10:00", "11:00"),
            create_event("second", "10:30", "11:30"),
            create_event("third", "11:00", "12:00"),
            create_event("separated", "12:00", "13:00"),
        ]);

        let start = create_date("2025-11-03");
        let lanes = find_clashes(&events, &start, short_event_clash_condition);
        let [
            first_event_lane,
            second_event_lane,
            third_event_lane,
            separated_event_lane,
        ] = lanes.as_slice()
        else {
            panic!("find_clashes must return a vector of 3 elements");
        };

        assert!(matches!(first_event_lane, (0, 2)));
        assert!(matches!(second_event_lane, (1, 2)));
        assert!(matches!(third_event_lane, (0, 2)));
        assert!(matches!(separated_event_lane, (0, 1)));
    }

    //#[test]
    //fn test_long_event_clash() {
    //    let create_event = |title: &str, start_date: &str, end_date: &str| Event {
    //        title: title.to_owned(),
    //        start_date: create_date(start_date),
    //        start_time: create_time("10:00"),
    //        end_date: create_date(end_date),
    //        end_time: create_time("10:00"),
    //        all_day: "False".to_owned(),
    //    };
    //
    //    let events: Vec<Event> = Vec::from_iter([
    //        create_event("first", "2025-11-03", "2025-11-05"),
    //        create_event("second", "2025-11-04", "2025-11-06"),
    //        create_event("third", "2025-11-05", "2025-11-07"),
    //        create_event("separated", "2025-11-07", "2025-11-08"),
    //    ]);
    //
    //    let start = create_date("2025-11-03");
    //    let lanes = find_clashes(&events, &start, long_event_clash_condition);
    //    let [
    //        first_event_lane,
    //        second_event_lane,
    //        third_event_lane,
    //        separated_event_lane,
    //    ] = lanes.as_slice()
    //    else {
    //        panic!("find_clashes must return a vector of 3 elements");
    //    };
    //
    //    assert!(matches!(first_event_lane, (0, 2)), "{:?}", first_event_lane);
    //    assert!(matches!(second_event_lane, (1, 2)));
    //    assert!(matches!(third_event_lane, (0, 2)));
    //    assert!(matches!(separated_event_lane, (0, 1)));
    //}
}
