use super::{Date, DateStream, Event, Time};
use std::{ffi::OsStr, str::FromStr};
pub trait AgendaSource {
    type Data;
    type Error;
    fn obtain<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Self::Data, Self::Error>;
}

pub struct AgendaSourceStd;

impl AgendaSource for AgendaSourceStd {
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

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<Agenda, Self::Error>;
}

pub struct NanoSerde;
impl JsonParser for NanoSerde {
    type Error = nanoserde::DeJsonErr;

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<Agenda, Self::Error> {
        nanoserde::DeJson::deserialize_json(bytes)
    }
}

pub type Agenda = Vec<Event>;

#[derive(Debug)]
pub enum Error<PE> {
    Io(std::io::Error),
    InvalidUnicode(core::str::Utf8Error),
    Parse(PE),
    DurationIsTooBig,
}

const MAX_DURATION_DAYS: u8 = 35;
pub mod khal {
    use super::ObtainArguments;
    pub fn week_arguments(from: &str) -> ObtainArguments<'_> {
        ObtainArguments {
            from,
            duration_days: 7,
            backend_bin_path: "khal",
        }
    }
}

pub struct ObtainArguments<'s> {
    // date in the format YYYY-MM-DD
    pub from: &'s str,
    // date in the format YYYY-MM-DD
    pub duration_days: u8,
    // path to khal
    pub backend_bin_path: &'s str,
}

pub struct WeekSchedule {
    pub long_events: Agenda,
    pub short_events: Agenda,
}

pub fn obtain<AS, JP, O>(
    agenda_source: &AS,
    json_parser: &JP,
    arguments: &ObtainArguments,
) -> Result<WeekSchedule, Error<JP::Error>>
where
    AS: AgendaSource<Data = O, Error = std::io::Error>,
    JP: JsonParser,
    O: AsRef<[u8]>,
{
    if arguments.duration_days > MAX_DURATION_DAYS {
        return Err(Error::DurationIsTooBig);
    }

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
        arguments.from,
        &format!("{}d", arguments.duration_days),
    ];

    let data: AS::Data = agenda_source.obtain(&args).map_err(Error::Io)?;
    let bytes: &str = std::str::from_utf8(data.as_ref()).map_err(Error::InvalidUnicode)?;
    let mut week_schedule = WeekSchedule {
        short_events: Vec::new(),
        long_events: Vec::new(),
    };

    let date = Date::from_str(arguments.from).expect("the format of the date must be YYYY-MM-DD");
    let date_stream = DateStream::new(date).take(7);

    let agendas = bytes
        .split('\n')
        .take(7)
        .take_while(|p| !p.is_empty())
        .zip(date_stream);

    for item in agendas {
        let (agenda_json, date): (&str, Date) = item;
        let agenda: Agenda = json_parser.parse(agenda_json).map_err(Error::Parse)?;
        let event_items = agenda.into_iter().filter_map(|event: Event| {
            let event_type: EventType = determine_event_type(&event);
            match event_type {
                EventType::Short => Some((true, event)),
                EventType::Long => {
                    if event.start_date == date {
                        Some((false, event))
                    } else {
                        None
                    }
                }
                EventType::CrossNight => {
                    let cropped_event: Event = crop_event(&date, event);
                    Some((true, cropped_event))
                }
            }
        });

        for item in event_items {
            let (is_short, event): (bool, Event) = item;
            if is_short {
                week_schedule.short_events.push(event)
            } else {
                week_schedule.long_events.push(event)
            }
        }
    }

    Ok(week_schedule)
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
        }
    } else if date == &event.end_date {
        Event {
            start_date: event.end_date.clone(),
            start_time: Time::midnight(),
            end_date: event.end_date,
            end_time: event.end_time,
            title: event.title,
            all_day: event.all_day,
        }
    } else {
        panic!("only an event which shorter than 24 hours can be cropped")
    }
}

fn determine_event_type(event: &Event) -> EventType {
    let sd: &Date = &event.start_date;
    let ed: &Date = &event.end_date;
    let st: &Time = &event.start_time;
    let et: &Time = &event.end_time;
    let event_duration_days: i32 = ed.subtract(sd);
    assert!(event_duration_days >= 0);
    match event_duration_days {
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
