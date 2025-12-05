use crate::render;
use crate::date::Date;
use crate::EventData;

use super::TextCreate;
use super::render::RenderWeekCaptionsArgs;
use super::render::TextRender;
use super::render::{render_hours, render_weekdays};
use super::types::{FPoint, FRect};

pub struct Week<Text> {
    pub days: [Text; 7],
    pub hours: [Text; 24],
    pub dates: [Text; 7],
}

impl<Text> Week<Text> {
    pub fn render<TR, R>(&self, tr: &TR, args: &RenderWeekCaptionsArgs) -> impl Iterator<Item = R>
    where
        TR: TextRender<Result = R, Text = Text>,
    {
        let RenderWeekCaptionsArgs {
            hours_arguments,
            days_arguments,
            dates_arguments,
        } = args;
        render_weekdays(tr, self.days.iter(), days_arguments)
            .chain(render_hours(tr, self.hours.iter(), hours_arguments))
            .chain(render_weekdays(tr, self.dates.iter(), dates_arguments))
    }
}

/// create a structure with all of the texts for the week view.
///
/// # Panics
///
/// if `date_stream` does not provide 7 elements.
pub fn create_texts<TF, R, I, D>(text_factory: &TF, date_stream: I) -> Week<R>
where
    TF: TextCreate<Result = R>,
    I: Iterator<Item = D>,
    D: std::borrow::Borrow<super::date::Date>,
{
    let mut dates_iter = create_date_texts(text_factory, date_stream);
    let dates: [R; 7] = core::array::from_fn(|_| {
        dates_iter
            .next()
            .expect("date_stream didn't sufficient amount of elements")
    });

    Week {
        days: create_weekday_texts(text_factory),
        hours: create_hours_texts(text_factory),
        dates,
    }
}

pub fn create_hours_texts<TF, R>(text_factory: &TF) -> [R; 24]
where
    TF: TextCreate<Result = R>,
{
    let hours: [R; 24] = core::array::from_fn(|i| {
        let s = format!("{:02}:00", i);
        text_factory.text_create(s.as_str())
    });
    hours
}

pub fn create_weekday_texts<TF, R>(text_factory: &TF) -> [R; 7]
where
    TF: TextCreate<Result = R>,
{
    let weekdays = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let ret: [R; 7] = core::array::from_fn(|i| text_factory.text_create(weekdays[i]));
    ret
}

pub fn create_date_texts<TF, R, I, D>(text_factory: &TF, dates: I) -> impl Iterator<Item = R>
where
    TF: TextCreate<Result = R>,
    I: Iterator<Item = D>,
    D: std::borrow::Borrow<super::date::Date>,
{
    dates.map(|date| {
        let date: &super::date::Date = date.borrow();
        let text = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);
        text_factory.text_create(&text)
    })
}

pub fn create_event_title_texts<'text, 'tf, TF, R, I>(
    text_factory: &'tf TF,
    items: I,
) -> impl Iterator<Item = R>
where
    TF: TextCreate<Result = R> + 'tf,
    I: Iterator<Item = &'text str>,
{
    items.map(|text| text_factory.text_create(text))
}

pub struct View {
    pub event_surface: FRect,
    pub grid_rectangle: FRect,
    pub cell_width: f32,
    pub top_panel_height: f32,
    pub cell_height: f32,
}

impl View
{
    const EVENT_SURFACE_OFFSET_X: f32 = 100.;
    const EVENT_SURFACE_OFFSET_Y: f32 = 70.;

    pub fn new(
        window_size: FPoint,
        title_font_height: i32,
        long_lane_max_count: f32,
        long_events_count: usize,
    ) -> Self {
        let event_surface: FRect = {
            let x = 100.;
            let y = 70.;
            FRect {
                x: Self::EVENT_SURFACE_OFFSET_X,
                y: Self::EVENT_SURFACE_OFFSET_Y,
                w: window_size.y - y,
                h: window_size.x - x,
            }
        };

        let top_panel_height = (title_font_height + 15) as f32 * long_lane_max_count;
        let cell_width: f32 = event_surface.w / 7.;
        let grid_rectangle: FRect = {
            let grid_vertical_offset = if long_events_count == 0 {
                0f32
            } else {
                top_panel_height
            };

            FRect {
                x: event_surface.x,
                y: event_surface.y + grid_vertical_offset,
                w: event_surface.w,
                h: event_surface.h - grid_vertical_offset,
            }
        };

        let cell_height = grid_rectangle.h / 24.;
        Self {
            cell_height,
            cell_width,
            grid_rectangle,
            event_surface,
            top_panel_height,
        }
    }
}

pub fn create_short_event_rectangles(
    grid_rectangle: &FRect,
    short_events: &EventData,
    week_start: &Date,
) -> render::Rectangles {
    let arguments = render::Arguments {
        column_width: grid_rectangle.w / 7.,
        column_height: grid_rectangle.h,
        offset_x: grid_rectangle.x,
        offset_y: grid_rectangle.y,
    };

    render::short_event_rectangles(short_events, week_start, &arguments).collect()
}

pub fn create_long_event_rectangles(
    event_surface_rectangle: &FRect,
    long_events: &EventData,
    week_start: &Date,
    cell_width: f32,
    top_panel_height: f32,
) -> render::Rectangles {
    let arguments = render::Arguments {
        column_width: cell_width,
        column_height: top_panel_height,
        offset_x: event_surface_rectangle.x,
        offset_y: event_surface_rectangle.y,
    };

    let pinned_rectangles_res =
        render::long_event_rectangles(long_events, week_start, &arguments);

    pinned_rectangles_res.collect()
}

