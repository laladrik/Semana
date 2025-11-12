use super::Error;
use super::{Date, Event, Time};
use super::{MINUTES_PER_DAY, MINUTES_PER_HOUR};

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub struct Arguments {
    pub column_width: f32,
    pub column_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub struct EventText<'text, T> {
    pub text: &'text T,
    pub at: Point,
}

impl<'rect, 'ev, 'text, T> From<(&'rect Rectangle<'ev>, &'text T)> for EventText<'text, T> {
    fn from((rectangle, title): (&'rect Rectangle, &'text T)) -> Self {
        Self {
            text: title,
            at: Point {
                x: rectangle.at.x + 2.0,
                y: rectangle.at.y + 2.0,
            },
        }
    }
}

pub fn place_event_texts<'text, 'rect, 'ev, Text>(
    rectangles: &'rect [Rectangle<'ev>],
    event_titles: &'text [Text],
) -> impl Iterator<Item = EventText<'text, Text>>
where
    EventText<'text, Text>: From<(&'rect Rectangle<'ev>, &'text Text)>,
{
    rectangles
        .iter()
        .zip(event_titles.iter())
        .map(EventText::from)
}

pub fn event_texts<'text, I, TR, R, T>(tr: &TR, texts: I) -> impl Iterator<Item = R>
where
    TR: TextRender<Result = R, Text = T>,
    T: 'text,
    I: Iterator<Item = EventText<'text, T>>,
{
    texts.map(|t| tr.text_render(t.text, t.at.x, t.at.y))
}

pub trait TextRender {
    type Text;
    type Result;
    fn text_render(&self, text: &Self::Text, x: f32, y: f32) -> Self::Result;
}

pub fn render_weekdays<'text, TR, T: 'text, R>(
    tr: &TR,
    texts: impl Iterator<Item = &'text T>,
    arguments: &Arguments,
) -> impl Iterator<Item = R>
where
    TR: TextRender<Result = R, Text = T>,
{
    let Arguments {
        column_width,
        column_height: _column_height,
        offset_x,
        offset_y,
    } = arguments;

    texts.enumerate().map(move |(i, text)| {
        let x = *offset_x + (i as f32) * column_width;
        tr.text_render(text, x, *offset_y)
    })
}

pub struct RenderHoursArgs {
    pub row_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

pub fn render_hours<'text, TR, T: 'text, R>(
    tr: &TR,
    texts: impl Iterator<Item = &'text T>,
    arguments: &RenderHoursArgs,
) -> impl Iterator<Item = R>
where
    TR: TextRender<Result = R, Text = T>,
{
    let RenderHoursArgs {
        row_height,
        offset_x,
        offset_y,
    } = arguments;
    texts.enumerate().map(move |(i, text)| {
        let y = *offset_y + (i as f32) * row_height;
        tr.text_render(text, *offset_x, y)
    })
}

pub struct RenderWeekCaptionsArgs {
    pub hours_arguments: RenderHoursArgs,
    pub days_arguments: Arguments,
    pub dates_arguments: Arguments,
}

pub fn render_week_captions<'text, TR, TI, R, T: 'text>(
    tr: &TR,
    days: TI,
    hours: TI,
    dates: TI,
    args: &RenderWeekCaptionsArgs,
) -> impl Iterator<Item = R>
where
    TR: TextRender<Result = R, Text = T>,
    TI: Iterator<Item = &'text T>,
{
    let RenderWeekCaptionsArgs {
        hours_arguments,
        days_arguments,
        dates_arguments,
    } = args;
    render_weekdays(tr, days, days_arguments)
        .chain(render_hours(tr, hours, hours_arguments))
        .chain(render_weekdays(tr, dates, dates_arguments))
}

pub type Size = Point;

#[cfg_attr(test, derive(PartialEq))]
pub struct Rectangle<'s> {
    pub at: Point,
    pub size: Size,
    pub text: &'s str,
}

fn calculate_event_point_x(
    first_date: &Date,
    event_date: &Date,
    column_width: f32,
    offset_x: f32,
) -> f32 {
    let days = event_date.subtract(first_date);
    assert!(
        days >= 0,
        "the first date in the calendar must be earlier than the date of the event",
    );

    days as f32 * column_width + offset_x
}

fn create_point<'ev>(
    first_date: &'_ Date,
    start_date: &'ev Date,
    start_time: &'ev Time,
    arguments: &Arguments,
) -> Point {
    let Arguments {
        column_width,
        column_height,
        offset_x,
        offset_y,
    } = arguments;
    let x = calculate_event_point_x(first_date, start_date, *column_width, *offset_x);
    let y = (start_time.minutes_from_midnight() as f32 / MINUTES_PER_DAY as f32) * column_height
        + offset_y;
    Point { x, y }
}

pub type Rectangles<'ev> = Vec<Rectangle<'ev>>;

pub struct RectangleSet<'ev> {
    pub pinned: Rectangles<'ev>,
    pub scrolled: Rectangles<'ev>,
}

fn create_long_event_rectangle<'ev>(
    long_event: &'ev Event,
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Rectangle<'ev> {
    let Arguments {
        column_width,
        column_height,
        offset_x,
        offset_y,
    } = arguments;
    let calc_x = |date: &Date, time: &Time| -> f32 {
        let days = date.subtract(first_date);
        let day_tail = (time.hour as u16 * MINUTES_PER_HOUR as u16 + time.minute as u16) as f32;
        (day_tail / (MINUTES_PER_DAY as f32) + days as f32) * column_width + offset_x
    };

    let start_x = calc_x(&long_event.start_date, &long_event.start_time);
    let start_point = Point {
        x: start_x,
        y: *offset_y,
    };

    let size: Size = {
        let end_x = calc_x(&long_event.end_date, &long_event.end_time);
        Size {
            x: end_x - start_x,
            y: *column_height,
        }
    };

    Rectangle {
        at: start_point,
        size,
        text: &long_event.title,
    }
}

fn create_short_event_rectangle<'ev>(
    event: &'ev Event,
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Rectangle<'ev> {
    assert_eq!(event.start_date, event.end_date);
    let start_point: Point =
        create_point(first_date, &event.start_date, &event.start_time, arguments);
    let size: Size = {
        let end_point: Point =
            create_point(first_date, &event.end_date, &event.end_time, arguments);
        Size {
            x: arguments.column_width,
            y: end_point.y - start_point.y,
        }
    };

    Rectangle {
        at: start_point,
        size,
        text: &event.title,
    }
}

pub fn long_event_rectangles<'ev>(
    long_events: &'ev [Event],
    long_lanes: &'ev [(Lane, Lane)],
    first_date: &Date,
    arguments: &Arguments,
) -> impl Iterator<Item = Rectangle<'ev>> {
    long_events.iter().zip(long_lanes).map(|item| {
        let (event, lane_position): (&Event, &(Lane, Lane)) = item;
        let (event_lane, total_lanes) = *lane_position;
        let mut rect = create_long_event_rectangle(event, first_date, arguments);
        if total_lanes != 1 {
            let lane_height: f32 = arguments.column_height / total_lanes as f32;
            rect.at.y += lane_height * event_lane as f32;
            rect.size.y = lane_height;
        }
        rect
    })
}

/// Atasco means a traffic jam.  The structure represents a set of overlapping events.
#[derive(Default)]
struct Atasco<'ev> {
    rectangles: Rectangles<'ev>,
    lanes: Vec<Lane>,
    end: f32,
}

impl<'ev> Atasco<'ev> {
    fn flush(&mut self, into: &mut Rectangles<'ev>, cell_width: f32, lane_count: Lane) {
        let lane_width = cell_width / lane_count as f32;
        let iter = self
            .rectangles
            .drain(..)
            .zip(self.lanes.drain(..))
            .map(|(rect, lane)| Rectangle {
                at: Point {
                    x: rect.at.x + lane_width * lane as f32,
                    y: rect.at.y,
                },
                size: Point {
                    x: lane_width,
                    y: rect.size.y,
                },
                text: rect.text,
            });
        into.extend(iter);
        self.end = f32::default();
    }

    fn push(&mut self, rect: Rectangle<'ev>, lane: Lane) {
        self.end = f32::max(rect.at.y + rect.size.y, self.end);
        self.rectangles.push(rect);
        self.lanes.push(lane);
    }
}

type Lane = u8;

fn find_free_lane(new_event_begin: f32, atasco: &Atasco) -> (Lane, f32) {
    assert!(
        new_event_begin >= 0.,
        "the beginning of the new event must not be nagetive"
    );
    let (index, end) = atasco
        .rectangles
        .iter()
        .map(|rect| rect.at.y + rect.size.y)
        .enumerate()
        .filter(|(_, end)| *end <= new_event_begin)
        .fold((0usize, f32::NEG_INFINITY), |acc, item| {
            let (acc_index, acc_end) = acc;
            let (index, end): (usize, f32) = item;
            let diff = new_event_begin - end;
            let acc_diff = new_event_begin - acc_end;
            if diff <= acc_diff && diff >= 0. {
                (index, end)
            } else {
                (acc_index, acc_end)
            }
        });

    if end.is_infinite() {
        (0, end)
    } else {
        let lane = unsafe { atasco.lanes.get_unchecked(index) };
        (*lane, end)
    }
}

/// Creates rectangles which visualize the position of the `events`.
///
/// # Assumptions
/// The `events` are sorted by [`Event::start_time`]
pub fn short_event_rectangles<'ev>(
    short_events: &'ev [Event],
    short_lanes: &[(Lane, Lane)],
    first_date: &'_ Date,
    arguments: &Arguments,
) -> impl Iterator<Item = Rectangle<'ev>> {
    for event in short_events {
        assert_eq!(event.start_date, event.end_date);
    }

    short_events.iter().zip(short_lanes).map(|item| {
        let (event, lane_position): (&Event, &(Lane, Lane)) = item;
        let (event_lane, total_lanes) = *lane_position;
        let mut rect = create_short_event_rectangle(event, first_date, arguments);
        if total_lanes != 1 {
            let column_width: f32 = arguments.column_width;
            let lane_width: f32 = column_width / total_lanes as f32;
            rect.size.x = lane_width;
            rect.at.x += event_lane as f32 * lane_width;
        }
        rect
    })
}

pub trait RenderRectangles {
    type Result;
    fn render_rectangles<'r, 's: 'r, I>(&self, data: I) -> Self::Result
    where
        I: Iterator<Item = &'r Rectangle<'s>>;
}

pub fn render_rectangles<'r, 's: 'r, I, DR, R>(rectangles: I, dr: &DR) -> R
where
    DR: RenderRectangles<Result = R>,
    I: Iterator<Item = &'r Rectangle<'s>>,
{
    dr.render_rectangles(rectangles)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::render::short_event_rectangles;

    #[track_caller]
    fn assert_approx_f32(left: f32, right: f32, tolerance: f32) {
        let diff = left - right;
        assert!(
            diff.abs() < tolerance,
            "{} must be similar to {}",
            left,
            right
        );
    }

    #[test]
    fn test_create_long_event_rectangle() {
        let event = Event {
            title: "all day event".to_owned(),
            start_date: create_date("2025-11-04"),
            start_time: create_time("00:00"),
            end_date: create_date("2025-11-06"),
            end_time: create_time("00:00"),
            all_day: "False".to_owned(),
        };

        let first_date: Date = create_date("2025-11-03");
        let arguments = Arguments {
            column_width: 100.,
            column_height: 50.,
            offset_x: 125.,
            offset_y: 70.,
        };

        let rectangle: Rectangle<'_> = create_long_event_rectangle(&event, &first_date, &arguments);

        let expected_x: f32 = arguments.offset_x + arguments.column_width * 1.;
        assert_eq!(rectangle.at.x, expected_x);
        let end: f32 = arguments.offset_x + arguments.column_width * 3.;
        let expected_width: f32 = end - expected_x;
        assert_eq!(rectangle.size.x, expected_width);
    }

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
    fn test_top_left_event() {
        let events = [Event {
            all_day: "False".to_owned(),
            title: "arst".to_owned(),
            start_date: create_date("2025-09-29"),
            start_time: create_time("00:00"),
            end_date: create_date("2025-09-29"),
            end_time: create_time("01:00"),
        }];

        let arguments = Arguments {
            column_width: 125.0,
            column_height: 600.0,
            offset_x: 0.,
            offset_y: 0.,
        };

        let start_date = Date {
            year: 2025,
            month: 9,
            day: 29,
        };

        let lanes = Vec::from([(0, 1)]);
        let ret: Rectangles =
            short_event_rectangles(&events, &lanes, &start_date, &arguments).collect();
        const ONE_HOUR: f32 = 600.0 / 24.0;
        let [x] = ret.as_slice() else {
            panic!("there must be a single rectangle");
        };
        assert!(
            matches!(
                x,
                Rectangle {
                    at: Point { x: 0.0, y: 0.0 },
                    size: Point {
                        x: 125.0,
                        y: ONE_HOUR
                    },
                    ..
                },
            ),
            "the actual at {:?}, the actual size {:?}",
            x.at,
            x.size,
        );
    }
}
