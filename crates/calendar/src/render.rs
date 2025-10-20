use core::str::FromStr;

use super::Date;
use super::Item as AgendaItem;
use super::MINUTES_PER_DAY;
use super::Time;
use super::Error;

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct Point {
    pub x: f32,
    pub y: f32,
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
    event_date: &'ev str,
    event_time: &'ev str,
    arguments: &Arguments,
) -> Result<Point, Error<'ev>> {
    let Arguments {
        column_width,
        column_height,
        offset_x,
        offset_y,
    } = arguments;
    let start_date: Date =
        Date::from_str(event_date).map_err(|_| Error::InvalidDate(event_date))?;
    let start_time: Time =
        Time::from_str(event_time).map_err(|_| Error::InvalidTime(event_time))?;

    let x = calculate_event_point_x(first_date, &start_date, *column_width, *offset_x);
    let y = (start_time.minutes_from_midnight() as f32 / MINUTES_PER_DAY as f32) * column_height
        + offset_y;
    Ok(Point { x, y })
}

pub type Rectangles<'ev> = Vec<Rectangle<'ev>>;

fn is_all_day(event: &AgendaItem) -> bool {
    event.all_day == "True"
}

fn not_all_day(event: &AgendaItem) -> bool {
    event.all_day == "False"
}

pub struct RectangleSet<'ev> {
    pub pinned: Rectangles<'ev>,
    pub scrolled: Rectangles<'ev>,
}

fn create_whole_day_rectangle<'ev>(
    event: &'ev AgendaItem,
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Result<Rectangle<'ev>, Error<'ev>> {
    let Arguments {
        column_width,
        column_height,
        offset_x,
        offset_y,
    } = arguments;
    let start_date: Date =
        Date::from_str(&event.start_date).map_err(|_| Error::InvalidDate(&event.start_date))?;
    let x = calculate_event_point_x(first_date, &start_date, *column_width, *offset_x);
    let start_point = Point { x, y: *offset_y };

    let size: Size = {
        let x = calculate_event_point_x(first_date, &start_date, *column_width, *offset_x);
        let end_point = Point {
            x,
            y: offset_y + column_height,
        };

        calculate_size(&start_point, &end_point, arguments.column_width)
    };

    Ok(Rectangle {
        at: start_point,
        size,
        text: &event.title,
    })
}

fn create_day_rectangle<'ev>(
    event: &'ev AgendaItem,
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Result<Rectangle<'ev>, Error<'ev>> {
    let start_point: Point =
        create_point(first_date, &event.start_date, &event.start_time, arguments)?;
    let size: Size = {
        let end_point: Point =
            create_point(first_date, &event.end_date, &event.end_time, arguments)?;
        calculate_size(&start_point, &end_point, arguments.column_width)
    };

    Ok(Rectangle {
        at: start_point,
        size,
        text: &event.title,
    })
}

fn calculate_size(start_point: &Point, end_point: &Point, column_width: f32) -> Size {
    Size {
        x: end_point.x - start_point.x + column_width,
        y: end_point.y - start_point.y,
    }
}

pub fn whole_day_rectangles<'ev>(
    events: &'ev [AgendaItem],
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Result<Rectangles<'ev>, Error<'ev>> {
    let mut ret = Vec::new();
    for whole_day_event in events.iter().filter(|x| is_all_day(x)) {
        let rect = create_whole_day_rectangle(whole_day_event, first_date, arguments)?;
        ret.push(rect);
    }
    Ok(ret)
}

pub fn event_rectangles<'ev>(
    events: &'ev [AgendaItem],
    first_date: &'_ Date,
    arguments: &Arguments,
) -> Result<Rectangles<'ev>, Error<'ev>> {
    let mut ret = Vec::new();
    for event in events.iter().filter(|x| not_all_day(x)) {
        let rect = create_day_rectangle(event, first_date, arguments)?;
        ret.push(rect);
    }
    Ok(ret)
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
    use super::*;
    use crate::render::event_rectangles;

    #[test]
    fn top_left_event() {
        let events = [AgendaItem {
            title: "arst".to_owned(),
            start_date: "2025-09-29".to_owned(),
            start_time: "00:00".to_owned(),
            end_date: "2025-09-29".to_owned(),
            end_time: "01:00".to_owned(),
        }];

        let arguments = Arguments {
            column_width: 125.0,
            column_height: 600.0,
            offset_x: 0.,
            offset_y: 0.,
        };

        let ret: Result<Rectangles, Error> = event_rectangles(&events, &arguments);
        const ONE_HOUR: f32 = 600.0 / 24.0;
        match ret {
            Ok(x) => assert!(
                matches!(
                    x[0],
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
                x[0].at,
                x[0].size,
            ),
            Err(e) => panic!(
                "the rectangles must be built.  However, the error occurred: {:?}",
                e
            ),
        }
    }
}
