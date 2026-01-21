use crate::EventData;
use crate::Lane;
use crate::date::Date;
use crate::render;

use super::TextCreate;
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

/// The trait is effectively a module with compile-time arguments.  It converts the input strings
/// into the text objects to be rendered.  The creation of a text object is defined by Backend.
pub trait TextObjectFactory {
    /// The result of Backend work.  It's conceived that it contains the text object to be
    /// rendered.
    type BackendResult;
    /// The implementation of creation of a text object.
    type Backend: TextCreate<Result = Self::BackendResult>;

    /// Create the texts for the week calendar.  See [`Week`].
    ///
    /// # Panics
    ///
    /// if `date_stream` does not provide 7 elements.
    fn create_texts<I, D>(text_factory: &Self::Backend, date_stream: I) -> Week<Self::BackendResult>
    where
        I: Iterator<Item = D>,
        D: std::borrow::Borrow<super::date::Date>,
    {
        let mut dates_iter = Self::create_date_texts(text_factory, date_stream);
        let dates: [Self::BackendResult; 7] = core::array::from_fn(|_| {
            dates_iter
                .next()
                .expect("date_stream didn't sufficient amount of elements")
        });

        Week {
            days: Self::create_weekday_texts(text_factory),
            hours: Self::create_hours_texts(text_factory),
            dates,
        }
    }

    fn create_hours_texts(text_factory: &Self::Backend) -> [Self::BackendResult; 24] {
        core::array::from_fn(|i| {
            let s = format!("{:02}:00", i);
            text_factory.text_create(s.as_str())
        })
    }

    fn create_weekday_texts(text_factory: &Self::Backend) -> [Self::BackendResult; 7] {
        let weekdays = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];
        core::array::from_fn(|i| text_factory.text_create(weekdays[i]))
    }

    fn create_date_texts<I, D>(
        text_factory: &Self::Backend,
        dates: I,
    ) -> impl Iterator<Item = Self::BackendResult>
    where
        I: Iterator<Item = D>,
        D: std::borrow::Borrow<super::date::Date>,
    {
        dates.map(|date| {
            let date: &super::date::Date = date.borrow();
            let text = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);
            text_factory.text_create(&text)
        })
    }

    fn create_event_title_texts<'text, 'tf>(
        text_factory: &'tf Self::Backend,
        items: impl Iterator<Item = &'text str>,
    ) -> impl Iterator<Item = Self::BackendResult> {
        items.map(|text| text_factory.text_create(text))
    }
}

pub struct UI<TF, R> {
    _marker: std::marker::PhantomData<(TF, R)>,
}

impl<TF, R> TextObjectFactory for UI<TF, R>
where
    TF: TextCreate<Result = R>,
{
    type BackendResult = R;
    type Backend = TF;
}

pub struct View {
    /// The rectangle which displays the short events and long events.
    pub event_surface: FRect,
    /// The rectangle which displays the short events.
    pub grid_rectangle: FRect,
    /// The width of cell on the grid containing the events.
    pub cell_width: f32,
    pub cell_height: f32,
    pub top_panel_height: f32,
}

pub struct SurfaceAdjustment {
    pub vertical_scale: f32,
    pub vertical_offset: f32,
}

impl View {
    const LINE_HEIGHT: u8 = 15;
    pub fn new(
        viewport_size: FPoint,
        // mutable for the case when we zoom out enough to make a gap between the bottom of the
        // viewport of the grid and the bottom of the grid.  Other words, the adjustment changes if
        // there empty space as a result of zooming out.
        adjustment: &mut SurfaceAdjustment,
        title_font_height: i32,
        long_event_clash_size: Lane,
    ) -> Self {
        let event_surface: FRect = {
            FRect {
                x: 0f32,
                y: 0f32,
                w: viewport_size.x,
                h: viewport_size.y + adjustment.vertical_scale,
            }
        };

        // The panel above the grid with the short events and beyond the days.
        let top_panel_height =
            (title_font_height + Self::LINE_HEIGHT as i32) as f32 * long_event_clash_size as f32;
        let cell_width: f32 = event_surface.w / 7.;
        let grid_rectangle: FRect = {
            let create = |offset| FRect {
                x: event_surface.x,
                y: event_surface.y + top_panel_height + offset,
                w: event_surface.w,
                h: event_surface.h - top_panel_height,
            };

            let mut ret = create(adjustment.vertical_offset);
            let bottom = ret.y + ret.h;
            let bottom_gap = viewport_size.y - bottom;
            if bottom_gap.is_sign_positive() {
                adjustment.vertical_offset += bottom_gap;
                ret = create(adjustment.vertical_offset);
            }
            ret
        };

        let cell_height = grid_rectangle.h / 24.;
        Self {
            top_panel_height,
            cell_height,
            cell_width,
            grid_rectangle,
            event_surface,
        }
    }

    #[inline(always)]
    pub fn calculate_top_panel_height(&self) -> f32 {
        self.top_panel_height
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

    let pinned_rectangles_res = render::long_event_rectangles(long_events, week_start, &arguments);

    pinned_rectangles_res.collect()
}

pub struct CursorPosition<T> {
    inner: FPoint,
    _phantom: std::marker::PhantomData<T>,
}

impl CursorPosition<Window> {
    pub fn in_window(x: f32, y: f32) -> CursorPosition<Window> {
        CursorPosition {
            inner: FPoint { x, y },
            _phantom: std::marker::PhantomData::<Window>,
        }
    }
}

impl CursorPosition<Surface> {
    pub fn in_surface(
        cursor_position: CursorPosition<Window>,
        surface: &FRect,
    ) -> CursorPosition<Surface> {
        CursorPosition::<Surface> {
            inner: FPoint {
                x: cursor_position.inner.x - surface.x,
                y: cursor_position.inner.y - surface.y,
            },
            _phantom: std::marker::PhantomData::<Surface>,
        }
    }
}

pub struct Surface;
pub struct Window;

impl From<FPoint> for CursorPosition<Surface> {
    fn from(inner: FPoint) -> Self {
        Self {
            inner,
            _phantom: std::marker::PhantomData::<Surface>,
        }
    }
}

pub struct RelativeOffset(f32);

pub struct EventSurface;
impl EventSurface {
    pub fn calculate_surface_offset(surface: &mut FRect, cursor: &CursorPosition<Surface>) {
        let offset = Self::calculate_cursor_relative_vertical_offset(surface, cursor);
        Self::calculate_absolute_vertical_offset(surface, cursor, offset);
    }

    fn calculate_cursor_relative_vertical_offset(
        surface: &FRect,
        cursor_position: &CursorPosition<Surface>,
    ) -> RelativeOffset {
        RelativeOffset(cursor_position.inner.y / surface.h)
    }

    fn calculate_absolute_vertical_offset(
        surface: &mut FRect,
        cursor: &CursorPosition<Surface>,
        cursor_offset: RelativeOffset,
    ) {
        // the position of the cursor where it could be
        let RelativeOffset(cursor_offset) = cursor_offset;
        let yp = surface.h * cursor_offset;
        surface.y -= yp - cursor.inner.y;
    }
}

//pub fn zoom_event_surface(event: ZoomEvent) {}

pub struct ZoomEvent {
    pub cursor_position: FPoint,
    pub scroll_size: f32,
}
