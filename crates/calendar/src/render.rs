use core::str::FromStr;

use super::Date;
use super::Item as AgendaItem;
use super::MINUTES_PER_DAY;
use super::TextCreate;
use super::Time;

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
pub struct Rectange<'s> {
    pub at: Point,
    pub size: Size,
    pub text: &'s str,
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

    let days = start_date.subtract(first_date);
    assert!(
        days >= 0,
        "the first date in the calendar must be earlier than the date of the event",
    );

    let x = days as f32 * column_width + offset_x;
    let y = (start_time.minutes_from_midnight() as f32 / MINUTES_PER_DAY as f32) * column_height
        + offset_y;
    Ok(Point { x, y })
}

#[derive(Debug)]
pub enum Error<'s> {
    InvalidDate(&'s str),
    InvalidTime(&'s str),
}

pub type Rectangles<'ev> = Vec<Rectange<'ev>>;

fn is_all_day(event: &AgendaItem) -> bool {
    event.all_day == "True"
}

fn not_all_day(event: &AgendaItem) -> bool {
    event.all_day == "False"
}

pub fn event_rectangles<'ev>(
    events: &'ev [AgendaItem],
    arguments: &Arguments,
) -> Result<Rectangles<'ev>, Error<'ev>> {
    let mut ret = Vec::new();

    let first_date: Date = match events.first() {
        Some(x) => {
            let start_date: &str = x.start_date.as_str();
            let date: Date = match Date::from_str(start_date) {
                Ok(x) => x,
                Err(_) => return Err(Error::InvalidDate(start_date)),
            };
            date
        }
        None => return Ok(ret),
    };

    for event in events.iter().filter(|x| not_all_day(x)) {
        let start_point: Point =
            create_point(&first_date, &event.start_date, &event.start_time, arguments)?;
        let size: Size = {
            let end_point: Point =
                create_point(&first_date, &event.end_date, &event.end_time, arguments)?;

            Size {
                x: end_point.x - start_point.x + arguments.column_width,
                y: end_point.y - start_point.y,
            }
        };

        ret.push(Rectange {
            at: start_point,
            size,
            text: &event.title,
        });
    }
    Ok(ret)
}

pub trait RenderRectangles {
    type Result;
    fn render_rectangles<'r, 's: 'r, I>(&self, data: I) -> Self::Result
    where
        I: Iterator<Item = &'r Rectange<'s>>;
}

pub fn render_rectangles<'r, 's: 'r, I, DR, R>(rectangles: I, dr: &DR) -> R
where
    DR: RenderRectangles<Result = R>,
    I: Iterator<Item = &'r Rectange<'s>>,
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
                    Rectange {
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
