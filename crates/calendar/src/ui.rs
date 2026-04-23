use crate::EventData;
use crate::Lane;
use crate::date::Date;
use crate::render;

use super::render::RenderWeekCaptionsArgs;
use super::render::TextRender;
use super::render::{render_hours, render_weekdays};
use super::types::{FPoint, FRect};

/// The structure contains the text objects to be rendered.  See
/// [`TextObjectFactory::BackendResult`].
pub struct Week<Text> {
    /// The names of the days of the week. (E.g.  Sunday, Monday etc.)
    pub days: [Text; 7],
    /// The hours through out a day. (E.g. 00:00, 01:00, 02:00 etc.)
    pub hours: [Text; 24],
    /// The dates of the days of the week.  (E.g.  2025-11-17, 2025-11-18 etc.)
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

pub struct UI<TF, R> {
    _marker: core::marker::PhantomData<(TF, R)>,
}

pub struct SurfaceAdjustment {
    /// The absolute value in pixels.  The value is _added_ to the height of the grid with the short events.
    pub vertical_scale: f32,
    pub vertical_offset: f32,
}

pub struct View {
    /// The rectangle which displays the short events.
    pub short_event_surface: FRect,
    /// The width of cell on the grid containing the events.  Other words is the width of a long
    /// and a short event.
    pub cell_width: f32,
    /// The height of a short event.
    pub cell_height: f32,
    /// The height of the surface with the long events.
    pub long_event_surface_height: f32,
}

#[inline]
pub fn compute_event_surface(
    viewport_size: &FPoint,
    vertical_scale: f32,
    vertical_offset: f32,
) -> FRect {
    FRect {
        x: 0f32,
        y: vertical_offset,
        w: viewport_size.x,
        h: viewport_size.y + vertical_scale,
    }
}

impl View {
    #[inline]
    pub fn compute_top_panel_height(title_font_height: i32, long_event_clash_size: Lane) -> f32 {
        (title_font_height + Self::LINE_HEIGHT as i32) as f32 * long_event_clash_size as f32
    }

    const LINE_HEIGHT: u8 = 15;
    pub fn new(
        viewport_size: FPoint,
        adjustment: &SurfaceAdjustment,
        title_font_height: i32,
        long_event_clash_size: Lane,
    ) -> Self {
        let event_surface: FRect = compute_event_surface(
            &viewport_size,
            adjustment.vertical_scale,
            adjustment.vertical_offset,
        );
        // The panel above the grid with the short events and beyond the days.
        let top_panel_height =
            Self::compute_top_panel_height(title_font_height, long_event_clash_size);
        let cell_width: f32 = event_surface.w / 7.;

        let cell_height = event_surface.h / 24.;
        Self {
            long_event_surface_height: top_panel_height,
            cell_height,
            cell_width,
            short_event_surface: event_surface,
        }
    }

    #[inline(always)]
    pub fn calculate_top_panel_height(&self) -> f32 {
        self.long_event_surface_height
    }
}

pub fn create_short_event_rectangles(
    short_event_surface: &FRect,
    short_events: &EventData,
    week_start: &Date,
) -> render::Rectangles {
    let arguments = render::Arguments {
        column_width: short_event_surface.w / 7.,
        column_height: short_event_surface.h,
        offset_x: short_event_surface.x,
        offset_y: short_event_surface.y,
    };

    render::short_event_rectangles(short_events, week_start, &arguments).collect()
}

pub fn create_long_event_rectangles(
    offset: &FPoint,
    long_events: &EventData,
    week_start: &Date,
    cell_width: f32,
    top_panel_height: f32,
) -> render::Rectangles {
    let arguments = render::Arguments {
        column_width: cell_width,
        column_height: top_panel_height,
        offset_x: offset.x,
        offset_y: offset.y,
    };

    let pinned_rectangles_res = render::long_event_rectangles(long_events, week_start, &arguments);

    pinned_rectangles_res.collect()
}
