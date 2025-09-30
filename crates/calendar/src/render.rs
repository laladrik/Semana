use core::str::FromStr;

use super::Date;
use super::Item as AgendaItem;
use super::MINUTES_PER_DAY;
use super::Time;

#[cfg_attr(test, derive(PartialEq))]
pub struct Point {
    x: f32,
    y: f32,
}

pub type Size = Point;

#[cfg_attr(test, derive(PartialEq))]
pub struct Rectange<'s> {
    pub at: Point,
    pub size: Size,
    pub text: &'s str,
}

pub struct Arguments {
    column_width: u32,
    column_height: u32,
}

fn create_point<'ev>(
    first_date: &'_ Date,
    event_date: &'ev str,
    event_time: &'ev str,
    column_width: u32,
    height: u32,
) -> Result<Point, Error<'ev>> {
    let start_date: Date =
        Date::from_str(event_date).map_err(|_| Error::InvalidDate(event_date))?;
    let start_time: Time =
        Time::from_str(event_time).map_err(|_| Error::InvalidTime(event_time))?;

    let days = start_date.subtract(first_date);
    assert!(
        days >= 0,
        "the first date in the calendar must be earlier than the date of the event",
    );

    let x = days as f32 * column_width as f32;
    let y = (start_time.minutes_from_midnight() as f32 / MINUTES_PER_DAY as f32) * height as f32;
    Ok(Point { x, y })
}

#[cfg_attr(test, derive(Debug))]
pub enum Error<'s> {
    InvalidDate(&'s str),
    InvalidTime(&'s str),
}

pub type Rectangles<'ev> = Vec<Rectange<'ev>>;

pub fn into_rectangles<'ev>(
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

    for event in events {
        let start_point: Point = create_point(
            &first_date,
            &event.start_date,
            &event.start_time,
            arguments.column_width,
            arguments.column_height,
        )?;
        let size: Size = {
            let end_point: Point = create_point(
                &first_date,
                &event.start_date,
                &event.start_time,
                arguments.column_width,
                arguments.column_height,
            )?;

            Size {
                x: end_point.x - start_point.x,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::into_rectangles;

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
            column_width: 125,
            column_height: 600,
        };
        let ret: Result<Rectangles, Error> = into_rectangles(&events, &arguments);
        match ret {
            Ok(x) => assert!(matches!(
                x[0],
                Rectange {
                    at: Point { x: 0.0, y: 0.0 },
                    ..
                }
            )),
            Err(e) => panic!("the rectangles must be built.  However, the error occurred: {:?}", e),
        }
    }
}
