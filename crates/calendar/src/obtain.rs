use crate::{EventRange, MINUTES_PER_DAY};

use super::{Date, DateStream, DateString, Event, EventData, Minutes, Time};
use std::ffi::OsStr;
pub trait EventSource {
    type Data;
    type Error;
    fn obtain<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Self::Data, Self::Error>;
}

pub struct EventSourceStd;

impl EventSource for EventSourceStd {
    type Data = Vec<u8>;
    type Error = std::io::Error;

    fn obtain<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Self::Data, Self::Error> {
        use std::process;
        let mut cmd = process::Command::new(&args[0]);
        cmd.args(args[1..].iter());
        cmd.stdout(process::Stdio::piped());
        let child: process::Child = cmd.spawn()?;
        let output: process::Output = child.wait_with_output()?;
        if !output.status.success() {
            panic!("the command failed");
        }
        Ok(output.stdout)
    }
}

pub trait JsonParser {
    type Error;

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<EventVec, Self::Error>;
}

pub struct NanoSerde;
impl JsonParser for NanoSerde {
    type Error = nanoserde::DeJsonErr;

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<EventVec, Self::Error> {
        nanoserde::DeJson::deserialize_json(bytes)
    }
}

pub type EventVec = Vec<Event>;

#[derive(Debug)]
pub enum Error<PE> {
    Io(std::io::Error),
    InvalidUnicode(core::str::Utf8Error),
    Parse(PE),
    DurationIsTooBig,
}

const MAX_DURATION_DAYS: u8 = 35;
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

pub fn events_with_lanes<AS, JP, O>(
    agenda_source: &AS,
    json_parser: &JP,
    arguments: &ObtainArguments,
) -> Result<WeekScheduleWithLanes, Error<JP::Error>>
where
    AS: EventSource<Data = O, Error = std::io::Error>,
    JP: JsonParser,
    O: AsRef<[u8]>,
{
    obtain(agenda_source, json_parser, arguments).map(|events| get_lanes(events, arguments.from))
}

fn obtain<AS, JP, O>(
    agenda_source: &AS,
    json_parser: &JP,
    arguments: &ObtainArguments,
) -> Result<Events, Error<JP::Error>>
where
    AS: EventSource<Data = O, Error = std::io::Error>,
    JP: JsonParser,
    O: AsRef<[u8]>,
{
    if arguments.duration_days > MAX_DURATION_DAYS {
        return Err(Error::DurationIsTooBig);
    }

    let from: DateString = arguments.from.iso_8601();
    let args = [
        arguments.backend_bin_path,
        "list",
        "--json",
        "title",
        "--json",
        "start-date",
        "--json",
        "start-time",
        "--json",
        "end-date",
        "--json",
        "end-time",
        "--json",
        "all-day",
        "--json",
        "calendar-color",
        from.as_str(),
        &format!("{}d", arguments.duration_days),
    ];

    let data: AS::Data = agenda_source.obtain(&args).map_err(Error::Io)?;
    let bytes: &str = std::str::from_utf8(data.as_ref()).map_err(Error::InvalidUnicode)?;
    let mut week_schedule = Events {
        short: Vec::new(),
        long: Vec::new(),
    };

    let date = arguments.from;
    let date_stream = DateStream::new(date.clone()).take(7);

    let agendas = bytes
        .split('\n')
        .take(7)
        .take_while(|p| !p.is_empty())
        .zip(date_stream);

    for item in agendas {
        let (agenda_json, date): (&str, Date) = item;
        let agenda: EventVec = json_parser.parse(agenda_json).map_err(Error::Parse)?;
        let event_items = agenda
            .into_iter()
            .filter_map(|event: Event| short_event_filter(event, &date));

        for item in event_items {
            let (is_short, event): (bool, Event) = item;
            if is_short {
                week_schedule.short.push(event)
            } else {
                week_schedule.long.push(event)
            }
        }
    }

    Ok(week_schedule)
}

impl EventData {
    pub fn calculate_biggest_clash(&self) -> Lane {
        self.lanes
            .iter()
            .map(|(_, total_lane_count)| *total_lane_count)
            .max()
            .unwrap_or(0)
    }
}

pub struct Events {
    long: EventVec,
    short: EventVec,
}

pub struct WeekScheduleWithLanes {
    pub long: EventData,
    pub short: EventData,
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

    let create = |event: Event| -> (EventRange, String) {
        let Event {
            title,
            start_date,
            start_time,
            end_date,
            end_time,
            all_day: _,
            calendar_color,
        } = event;
        let range = EventRange {
            start_date,
            start_time,
            end_date,
            end_time,
            calendar_color,
        };
        (range, title)
    };

    let (long_event_ranges, long_event_titles): (Vec<EventRange>, Vec<String>) =
        events.long.into_iter().map(create).unzip();

    let (short_event_ranges, short_event_titles): (Vec<EventRange>, Vec<String>) =
        events.short.into_iter().map(create).unzip();

    WeekScheduleWithLanes {
        long: EventData {
            event_ranges: long_event_ranges,
            titles: long_event_titles,
            lanes: long_lanes,
        },

        short: EventData {
            event_ranges: short_event_ranges,
            titles: short_event_titles,
            lanes: short_lanes,
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
        let start_day_diff: i32 = event.start_date.subtract(start_date);
        let end_day_diff: i32 = event.end_date.subtract(start_date);
        assert!((0..7).contains(&start_day_diff));
        assert!((0..7).contains(&end_day_diff));
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

fn short_event_filter(mut event: Event, date: &Date) -> Option<(bool, Event)> {
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
            let cropped_event: Event = crop_event(date, event);
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
fn crop_event(date: &Date, event: Event) -> Event {
    if date == &event.start_date {
        Event {
            start_date: event.start_date.clone(),
            start_time: event.start_time,
            end_date: event.start_date,
            end_time: Time::last_minute(),
            title: event.title,
            all_day: event.all_day,
            calendar_color: event.calendar_color,
        }
    } else if date == &event.end_date {
        Event {
            start_date: event.end_date.clone(),
            start_time: Time::midnight(),
            end_date: event.end_date,
            end_time: event.end_time,
            title: event.title,
            all_day: event.all_day,
            calendar_color: event.calendar_color,
        }
    } else {
        panic!("only an event which shorter than 24 hours can be cropped")
    }
}

fn determine_event_type(event: &Event, is_all_day: bool) -> EventType {
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
        let create_event = |title: &str, start_time: &str, end_time: &str| Event {
            calendar_color: crate::Color::BLACK,
            title: title.to_owned(),
            start_date: create_date("2025-11-03"),
            start_time: create_time(start_time),
            end_date: create_date("2025-11-03"),
            end_time: create_time(end_time),
            all_day: "False".to_owned(),
        };

        let events: Vec<Event> = Vec::from_iter([
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
