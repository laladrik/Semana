use crate::render::{EventViewRenderData, RenderData, WeekViewRenderData};
use core::cell::RefCell;

mod calendar_state;
use calendar::types::{
    AddFPoint, AsFPoint, CoversPoint, MoveFRect, SubFPoint, is_fpoint_between_points,
};

use calendar_state::CalendarState;
use calendar_state::EventRectangles;
use calendar_state::WeekData;

use calendar::{
    date::DateStream,
    ui::{SurfaceAdjustment, View},
};

use sdl3_sys::{SDL_FPoint as FPoint, SDL_FRect as FRect, SDL_Point as Point, SDL_Rect as Rect};
use sdlext::Color;

// FIXME(alex): remove this ASAP
const DESCRIPTION_TEXT_INDEX: usize = 3;
const TEXT_SCROLL_AMPLIFIER: f32 = 10.0;

mod captions {
    pub mod event_details_view {
        pub const TITLE: &str = "Title:";
        pub const DESCRIPTION: &str = "Description:";
        pub const FROM: &str = "From:";
        pub const UNTIL: &str = "Until:";
    }
}

/// An event changes its color upon being clicked.  The function computes the difference between
/// the given color and that color.  The difference will be either _positive_ or _negative_.  It's
/// negative be default. However, if the `color` is close to black, the diffirence is positive.
/// There is no reason to dim the color which dark as it is.
///
// FIXME(alex): the compiler doesn't generate SIMD instructions for the floating point
// calculations.
fn compute_clicked_calendar_event_color(color: &calendar::Color) -> calendar::ColorDiff {
    use core::array::from_fn;
    type Vec3 = [f32; 3];
    fn get_len(vec3: &Vec3) -> f32 {
        let sum: f32 = vec3.iter().map(|x| x * x).sum();
        sum.sqrt()
    }

    let max = 255f32;
    const RED_SHIFT: u32 = 24;
    const GREEN_SHIFT: u32 = 16;
    const BLUE_SHIFT: u32 = 8;
    const _ALPHA_SHIFT: u32 = 0;

    let color_vec: Vec3 = {
        let val = color.0;
        let r = (val & (0xff << RED_SHIFT)) >> RED_SHIFT;
        let g = (val & (0xff << GREEN_SHIFT)) >> GREEN_SHIFT;
        let b = (val & (0xff << BLUE_SHIFT)) >> BLUE_SHIFT;
        [r as f32 / max, g as f32 / max, b as f32 / max]
    };

    let inverted_color_vec: Vec3 = {
        let color_vec_len = get_len(&color_vec);
        let inverted_color_vec: Vec3 = from_fn(|i| -color_vec[i]);
        // 15% from the the entire spectre.  E.g. from R: 255, G: 255, B: 255
        const ADJUSTING_VECTOR_LENGTH: f32 = 0.15;
        let adjusted_len = ADJUSTING_VECTOR_LENGTH / color_vec_len;
        from_fn(|i| inverted_color_vec[i] * adjusted_len)
    };

    let ret: Vec3 = from_fn(|i| color_vec[i] + inverted_color_vec[i]);
    let adjustment_color: Vec3 = if ret.iter().any(|color_channel| *color_channel < 0f32) {
        from_fn(|i| -inverted_color_vec[i])
    } else {
        inverted_color_vec
    };

    calendar::ColorDiff(adjustment_color)
}

struct ClickedCalendarEvent {
    /// When the user click an event, the color of the event is the sum of this and
    /// `original_color`.
    color_diff: calendar::ColorDiff,
    /// Every calendar event belongs to its calendar.  The value of the field is the color of that
    /// calendar.
    original_color: calendar::Color,
    /// The rectangle of the calendar event.  It is relative to window coordinates.
    rectangle: FRect,
    /// The index of the calendar event in the table [`calendar::EventTable`].  The same index is
    /// used to accesses the rectangle from [`CalendarState`] to render the calendar event on the
    /// week view.
    index: u32,
    kind: CalendarEventKind,
}

// FIXME(alex): this structure carries the data for the week view.  Probably, it should be renamed
// to WeekView.
pub struct Calendar<F: Frontend> {
    _frontend: std::marker::PhantomData<F>,
    pub week_start: calendar::date::Date,
    pub is_week_switched: bool,
    state: CalendarState<<F::AgendaSource as AgendaSource>::RequestHandle>,
    /// Holds the information about the event which was under the mouse cursor upen the left click.
    clicked_event: Option<ClickedCalendarEvent>,
}

impl<F: Frontend> Calendar<F> {
    fn new(frontend: &F) -> Result<Self, F::Error> {
        let week_start: calendar::date::Date = frontend.get_current_week_start()?;
        let is_week_switched = false;
        let agenda_source_handle = frontend.agenda_source().request(&week_start)?;
        Ok(Self {
            _frontend: std::marker::PhantomData,
            week_start,
            state: CalendarState::Loading {
                agenda_source_handle,
            },
            is_week_switched,
            clicked_event: None,
        })
    }

    pub fn add_week(&mut self) {
        self.week_start = self.week_start.add_week();
        self.is_week_switched = true;
    }

    pub fn subtract_week(&mut self) {
        self.week_start = self.week_start.subtract_week();
        self.is_week_switched = true;
    }

    fn update_week_data(&mut self, frontend: &F) -> Result<(), F::Error> {
        self.state.switch(|current_state| match current_state {
            CalendarState::Loading {
                agenda_source_handle,
            } => {
                let src = frontend.agenda_source();
                src.cancel(&agenda_source_handle);
                let ret = frontend.agenda_source().request(&self.week_start);
                match ret {
                    Ok(x) => {
                        src.free(agenda_source_handle);
                        CalendarState::loading(x, None)
                    }
                    Err(e) => CalendarState::loading(agenda_source_handle, e.into()),
                }
            }
            CalendarState::Ready { .. } | CalendarState::Rendering { .. } => {
                let ret = frontend.agenda_source().request(&self.week_start);
                match ret {
                    Ok(x) => CalendarState::loading(x, None),
                    Err(e) => (current_state, Some(e)),
                }
            }
        })?;

        self.is_week_switched = false;
        Ok(())
    }

    pub fn request_render(&mut self) {
        use CalendarState::*;
        self.state.switch_infallible(|state| match state {
            x @ (Loading { .. } | Rendering { .. }) => x,
            Ready {
                week_data,
                long_event_clash_size,
                ..
            } => Rendering {
                week_data,
                long_event_clash_size,
            },
        });
    }

    // transition from the state "rendering" to the state "ready"
    fn get_ready<'frontend, 'view, 'a, EventTextObjectDB>(
        &mut self,
        view: &'view View,
        top_panel_height: f32,
        frontend: &'frontend mut EventTextObjectDB,
        event_title_offset: &'a FPoint,
        event_offset: &FPoint,
    ) -> Result<(), F::Error>
    where
        EventTextObjectDB: GetLongEventTextRegistry<Registry = F::TextTextureRegistry>
            + GetShortEventTextRegistry<Registry = F::TextTextureRegistry>,
    {
        use CalendarState::*;
        let week_start = &self.week_start;
        self.state.switch(move |current_state| match current_state {
            Loading { .. } | Ready { .. } => (current_state, None),
            Rendering {
                week_data,
                long_event_clash_size,
            } => {
                let reg = EventTitleRegistration {
                    event_title_offset,
                    text_registry: frontend.get_short_event_text_registry(),
                };

                let short_event_rectangles_opt =
                    create_short_events(&week_data.agenda.short, week_start, reg, view);

                let short_event_rectangles_opt = match short_event_rectangles_opt {
                    Ok(x) => x,
                    Err(e) => {
                        return (
                            Rendering {
                                week_data,
                                long_event_clash_size,
                            },
                            Some(e),
                        );
                    }
                };

                let reg = EventTitleRegistration {
                    event_title_offset,
                    text_registry: frontend.get_long_event_text_registry(),
                };
                let long_event_rectangles_opt = create_long_events(
                    &week_data.agenda.long,
                    week_start,
                    reg,
                    event_offset,
                    view.cell_width,
                    top_panel_height,
                );

                let long_event_rectangles_opt = match long_event_rectangles_opt {
                    Ok(x) => x,
                    Err(e) => {
                        return (
                            Rendering {
                                week_data,
                                long_event_clash_size,
                            },
                            Some(e),
                        );
                    }
                };

                (
                    CalendarState::Ready {
                        week_data,
                        long_event_clash_size,
                        short_event_rectangles_opt,
                        long_event_rectangles_opt,
                    },
                    None,
                )
            }
        })
    }

    #[allow(unused)]
    pub fn is_new_week_data_received(&self, frontend: &F) -> bool {
        match &self.state {
            CalendarState::Loading {
                agenda_source_handle,
            } => frontend.agenda_source().is_ready(agenda_source_handle),
            CalendarState::Ready { .. } | CalendarState::Rendering { .. } => false,
        }
    }

    fn get_rendering(&mut self, frontend: &F) {
        self.state
            .switch_infallible(|current_state| match current_state {
                CalendarState::Loading {
                    agenda_source_handle,
                } => {
                    let src = frontend.agenda_source();
                    if src.is_ready(&agenda_source_handle) {
                        let agenda: calendar::obtain::WeekScheduleWithLanes =
                            src.fetch(&agenda_source_handle, &self.week_start);
                        let week_data = WeekData { agenda };
                        let long_event_clash_size = week_data.agenda.long.calculate_biggest_clash();
                        src.free(agenda_source_handle);
                        CalendarState::Rendering {
                            week_data,
                            long_event_clash_size,
                        }
                    } else {
                        CalendarState::Loading {
                            agenda_source_handle,
                        }
                    }
                }
                x => x,
            });
    }
}

pub struct UserInterface {
    pub adjustment: SurfaceAdjustment,
    pub title_font_height: std::ffi::c_int,
    /// From where the all of the events are drawn.
    pub event_offset: FPoint,
    pub mouse_position: FPoint,
    pub event_title_offset: FPoint,
}

struct LongEventSurface {
    offset: FPoint,
    size: FPoint,
}

impl LongEventSurface {
    fn new(event_offset: &FPoint, window_size: &Point, long_event_surface_height: f32) -> Self {
        let long_event_viewport_offset = *event_offset;
        let long_event_viewport_size = FPoint {
            x: window_size.x as f32 - event_offset.x,
            y: long_event_surface_height,
        };

        Self {
            offset: long_event_viewport_offset,
            size: long_event_viewport_size,
        }
    }
}

/// The viewport through which the short event surface is seen.
struct ShortEventViewport {
    offset: FPoint,
    size: FPoint,
}

impl ShortEventViewport {
    fn new(event_offset: &FPoint, window_size: &Point, long_event_surface_height: f32) -> Self {
        let short_event_viewport_offset: FPoint =
            event_offset.add_fpoint(0f32, long_event_surface_height);
        let short_event_viewport_size: FPoint = window_size
            .as_fpoint()
            .sub_fpoint(short_event_viewport_offset.x, short_event_viewport_offset.y);
        Self {
            offset: short_event_viewport_offset,
            size: short_event_viewport_size,
        }
    }

    fn from_long_event_surface(long_event_surface: &LongEventSurface, window_size: &Point) -> Self {
        Self::new(
            &long_event_surface.offset,
            window_size,
            long_event_surface.size.y,
        )
    }

    fn into_rect(self) -> Rect {
        Rect {
            x: self.offset.x as i32,
            y: self.offset.y as i32,
            w: self.size.x as i32,
            h: self.size.y as i32,
        }
    }
}

impl UserInterface {
    fn new(
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
        event_title_offset: FPoint,
        mouse_position: FPoint,
    ) -> Self {
        // the values to scale and scroll the events grid (short events).
        let adjustment = SurfaceAdjustment {
            vertical_scale: 0.,
            vertical_offset: 0.,
        };

        Self {
            adjustment,
            title_font_height,
            event_offset,
            mouse_position,
            event_title_offset,
        }
    }

    pub fn add_adjustment(&mut self, value: f32) {
        let new_value = self.adjustment.vertical_offset - value;
        self.adjustment.vertical_offset = new_value.clamp(-self.adjustment.vertical_scale, 0f32);
    }

    /// To scale the surface with the short events, its vertical offset is to be adjusted as well
    /// for two reasons:
    /// 1. The event under the cursor must stay under the cursor.
    /// 2. Given the surface scrolled to the end, shrinking the surface must not cause a gap
    ///    between the bottom of the surface and its viewport.
    ///
    /// - See [`compute_cursor_adjustment`]
    /// - See [`scale_short_events`]
    fn compute_short_event_surface_adjustment(
        &self,
        long_event_surface_height: f32,
        scale_value: f32,
        window_size: &Point,
    ) -> SurfaceAdjustment {
        let short_event_viewport =
            ShortEventViewport::new(&self.event_offset, window_size, long_event_surface_height);

        let current_adjustment = &self.adjustment;
        let new_vertical_scale =
            NonNegativeF32::clip_from(current_adjustment.vertical_scale + scale_value);
        let diff: f32 = compute_cursor_adjustment(
            &self.mouse_position,
            new_vertical_scale,
            &short_event_viewport.offset,
            &short_event_viewport.size,
            current_adjustment,
        );

        let mouse_adjustment = SurfaceAdjustment {
            vertical_scale: current_adjustment.vertical_scale,
            vertical_offset: current_adjustment.vertical_offset - diff,
        };

        scale_short_events(
            &mouse_adjustment,
            &short_event_viewport.size,
            new_vertical_scale,
        )
    }
}

// The state of the application
pub struct App<F: Frontend> {
    pub calendar: Calendar<F>,
    pub ui: UserInterface,
    event_details_view: Option<EventDetailsView>,
}

struct Textbox {
    border_rect: FRect,
    cursor_rect: Option<FRect>,
    /// The following 3 fields allows the text to be selected (highlighted).  The field indicates
    /// that the text is being selected.
    ///
    /// `highlight_start` stores the offset from the highlighting starts.  `highlight_end` stores
    /// the end of it.  Both are -1 until the selection has started.  `highlight_end` can be -1
    /// alone if the user only clicks somewhere in the text without selecting it.
    is_highlighting: bool,
    highlight_start: i32,
    highlight_end: i32,
}

impl Textbox {
    fn new(rect: FRect) -> Textbox {
        Textbox {
            border_rect: rect,
            cursor_rect: None,
            is_highlighting: false,
            highlight_start: -1,
            highlight_end: -1,
        }
    }
}

struct EventDetailsView {
    description_textbox: Option<Textbox>,
    event_index: u32,
    event_kind: CalendarEventKind,
    /// The fields which stretch as the window does.
    flexible_fields: Box<[u32]>,
    /// The strings which are rendered inside the text fields.
    texts: Box<[Box<str>]>,
    /// The offsets of the text objects.  They are used for the case when a string is longer than
    /// its field (a.k.a. viewport).
    offsets: Box<[f32]>,
}

const DUMB_CELL_WIDTH: f32 = 130f32;

impl<F: Frontend> App<F> {
    pub fn new(
        frontend: &mut F,
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
        mouse_position: FPoint,
        event_title_offset: FPoint,
    ) -> Result<Self, F::Error> {
        let ui = UserInterface::new(
            title_font_height,
            event_offset,
            event_title_offset,
            mouse_position,
        );
        let calendar = Calendar::new(frontend)?;
        App::create_hours_text_objects(frontend, ui.event_offset.x)?;

        let cell_width = DUMB_CELL_WIDTH; // FIXME: the value must be calculated
        App::create_days_text_objects(frontend, cell_width)?;

        App::create_dates_text_objects(frontend, cell_width, &calendar.week_start)?;
        Ok(Self {
            calendar,
            ui,
            event_details_view: None,
        })
    }

    fn get_selected_event_desription(&self) -> Option<&str> {
        self.event_details_view
            .as_ref()
            .and_then(|view: &EventDetailsView| {
                let s = &self.calendar.state;
                let is_long = match view.event_kind {
                    CalendarEventKind::Long => true,
                    CalendarEventKind::Short => false,
                };

                s.get_event_table(is_long)
                    .and_then(|t| t.obtain_description(view.event_index))
            })
    }

    #[inline]
    fn compute_viewport_size(
        event_offset: &FPoint,
        window_size: &Point,
        long_event_surface_height: f32,
    ) -> FPoint {
        let yoffset = event_offset.y + long_event_surface_height;
        window_size.as_fpoint().sub_fpoint(event_offset.x, yoffset)
    }

    fn create_view(
        event_offset: &FPoint,
        adjustment: &SurfaceAdjustment,
        window_size: &Point,
        long_event_surface_height: f32,
    ) -> View {
        let size =
            Self::compute_viewport_size(event_offset, window_size, long_event_surface_height);
        View::new(size, adjustment)
    }

    fn reposition_hours_text_objects(frontend: &mut F, width: f32, view: &View) {
        let cell_height = view.cell_height;
        let offset_y = view.short_event_surface.y;
        let hours_registry: &mut _ = frontend.get_hours_text_registry();
        let values = (0..24).map(|hour| {
            let y = offset_y + (hour as f32) * cell_height;
            FRect {
                x: 0f32,
                y,
                w: width,
                h: cell_height,
            }
        });

        hours_registry.update_positions(values);
    }

    fn create_hours_text_objects(frontend: &mut F, panel_width: f32) -> Result<(), F::Error> {
        let hours_registry: &mut _ = frontend.get_hours_text_registry();
        hours_registry.clear();
        for i in 0..24 {
            let s = format!("{:02}:00", i);
            // FIXME: avoid this empty rectangles.  They are needed only to define the text wrap
            // length
            let position = FRect {
                x: 0f32,
                y: 0f32,
                w: panel_width,
                h: 0f32,
            };

            hours_registry.create(s.as_str(), Color::WHITE, position)?;
        }
        Ok(())
    }

    fn reposition_days_text_objects(frontend: &mut F, offset: f32, view: &View) {
        let cell_width = view.cell_width;
        let cell_height = view.cell_height;
        let positions = (0..7).map(|day| FRect {
            x: cell_width * day as f32,
            y: offset,
            w: cell_width,
            h: cell_height,
        });

        frontend
            .get_days_text_registry()
            .update_positions(positions);
    }

    fn reposition_dates_text_objects(frontend: &mut F, offset: f32, view: &View) {
        let cell_width = view.cell_width;
        let cell_height = view.cell_height;
        let positions = (0..7).map(|day| FRect {
            x: cell_width * day as f32,
            y: offset,
            w: cell_width,
            h: cell_height,
        });

        frontend
            .get_dates_text_registry()
            .update_positions(positions);
    }

    fn create_days_text_objects(frontend: &mut F, cell_width: f32) -> Result<(), F::Error> {
        let days_registry = frontend.get_days_text_registry();
        let weekdays = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];

        for day in weekdays {
            // FIXME: avoid this empty rectangles.  They are needed only to define the text wrap
            // length
            let position = FRect {
                x: 0f32,
                y: 0f32,
                w: cell_width,
                h: 0f32,
            };
            days_registry.create(day, Color::WHITE, position)?;
        }
        Ok(())
    }

    fn create_dates_text_objects(
        frontend: &mut F,
        cell_width: f32,
        week_start: &calendar::date::Date,
    ) -> Result<(), F::Error> {
        let dates_registry = frontend.get_dates_text_registry();

        let mut dates = DateStream::new(week_start.clone());
        for _ in 0..7 {
            let date = dates
                .next()
                .expect("the date stream must provide at least 7 dates");
            let text = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);
            // FIXME: avoid this empty rectangles.  They are needed only to define the text wrap
            // length
            let position = FRect {
                x: 0f32,
                y: 0f32,
                w: cell_width,
                h: 0f32,
            };
            dates_registry.create(text, Color::WHITE, position)?;
        }

        Ok(())
    }

    fn compute_long_event_height(&self) -> f32 {
        View::compute_long_event_surface_height(
            self.ui.title_font_height,
            self.calendar.state.get_long_event_clash_size(),
        )
    }

    fn compute_long_event_surface(&self, window_size: &Point) -> LongEventSurface {
        LongEventSurface::new(
            &self.ui.event_offset,
            window_size,
            self.compute_long_event_height(),
        )
    }

    pub fn create_week_view_render_data<'wdrect, 'frontend>(
        &'wdrect mut self,
        frontend: &'frontend mut F,
        window_size: Point,
        events: impl IntoIterator<Item = Action>,
    ) -> Result<NewState<'wdrect, 'frontend, F>, F::Error> {
        let mut event_mouse_click: Option<MouseEventClick> = None;
        // :userInputHandling
        for event in events {
            use Action::*;
            match event {
                MouseButtonUp {
                    position: mouse_position,
                } => {
                    // 1. Erase the color of clicked rectangle and the rectangle itself.
                    // 2. Trigger the clicking on the calendar event which was under the cursor
                    //    when the user pressed the mouse button.
                    if let Some(clicked_event) = self.calendar.clicked_event.take() {
                        self.calendar.state.set_color(
                            clicked_event.index,
                            clicked_event.kind.is_long(),
                            clicked_event.original_color,
                        );

                        let long_event_surface = self.compute_long_event_surface(&window_size);
                        if clicked_event.rectangle.covers_point(&mouse_position) {
                            event_mouse_click = try_register_mouse_click(
                                mouse_position,
                                &long_event_surface,
                                &window_size,
                            );
                        }
                    }
                }
                Escape => (),
                WindowResize => self.calendar.request_render(),
                Scroll { offset: value, .. } => {
                    self.ui.add_adjustment(value * -50.);
                    self.calendar.request_render();
                }
                Zoom(value) => {
                    let long_event_surface_height: f32 = self.compute_long_event_height();
                    self.ui.adjustment = self.ui.compute_short_event_surface_adjustment(
                        long_event_surface_height,
                        value,
                        &window_size,
                    );
                    self.calendar.request_render();
                }
                MouseMove {
                    x,
                    y,
                    pressed_button,
                } => {
                    self.ui.mouse_position = FPoint { x, y };
                    // If the cursor moves with the pressed left button, the event changes its
                    // color if the cursor is over the event, otherwise the color is the original.
                    // This is enabled only if the cursor was over the event upon the click.
                    if let Some(MouseButton::Left) = pressed_button
                        && let Some(clicked_event) = self.calendar.clicked_event.as_ref()
                    {
                        let color = if clicked_event
                            .rectangle
                            .covers_point(&self.ui.mouse_position)
                        {
                            clicked_event
                                .original_color
                                .adjust(&clicked_event.color_diff)
                        } else {
                            clicked_event.original_color
                        };

                        self.calendar.state.set_color(
                            clicked_event.index,
                            clicked_event.kind.is_long(),
                            color,
                        );
                    }
                }
                SubtractWeek => self.calendar.subtract_week(),
                AddWeek => self.calendar.add_week(),
                MouseButtonDown {
                    position: mouse_position,
                    button: MouseButton::Left,
                } => {
                    let long_event_surface = self.compute_long_event_surface(&window_size);
                    let mouse_click =
                        try_register_mouse_click(mouse_position, &long_event_surface, &window_size);

                    // This is "tagging" of the clicked event.  The saved information allows us to:
                    // 1. Change the color of the event based on the cursor position.  The cursor
                    //    stays over the calendar event, the event has the adjusted color.  The
                    //    cursor is dragged away, the event has its original color.
                    // 2. Show the view with the event details upon the button release as long as
                    //    the cursor is over the calendar event.  Otherwise nothing happens.
                    self.calendar.clicked_event = mouse_click.and_then(|mouse_click| {
                        let MouseEventClick {
                            event_kind,
                            position,
                        } = mouse_click;
                        let rectangles: EventRectangles = self.calendar.state.obtain_events();
                        let (is_long, rectangles): (_, _) = match event_kind {
                            CalendarEventKind::Long => (true, rectangles.long),
                            CalendarEventKind::Short => (false, rectangles.short),
                        };

                        find_clicked_event(&position, rectangles).and_then(|event: usize| {
                            self.calendar
                                .state
                                .get_rectangle(event as u32, is_long)
                                .map(|rectangle| {
                                    let origin: FRect = {
                                        let ret = FRect {
                                            x: rectangle.at.x,
                                            y: rectangle.at.y,
                                            w: rectangle.size.x,
                                            h: rectangle.size.y,
                                        };

                                        // FIXME(alex): this should in a function which would
                                        // "normalize" the coordinates of any event based on its
                                        // kind.
                                        let offset = match event_kind {
                                            CalendarEventKind::Long => FPoint { x: 0., y: 0. },
                                            CalendarEventKind::Short => {
                                                ShortEventViewport::from_long_event_surface(
                                                    &long_event_surface,
                                                    &window_size,
                                                )
                                                .offset
                                            }
                                        };

                                        ret.move_frect(offset.x, offset.y)
                                    };

                                    let color_diff =
                                        compute_clicked_calendar_event_color(&rectangle.color);
                                    ClickedCalendarEvent {
                                        index: event as u32,
                                        original_color: rectangle.color,
                                        rectangle: origin,
                                        kind: event_kind,
                                        color_diff,
                                    }
                                })
                        })
                    });

                    // Adjust the color of the clicked event.
                    if let Some(clicked_event) = self.calendar.clicked_event.as_ref() {
                        self.calendar.state.set_color(
                            clicked_event.index,
                            clicked_event.kind.is_long(),
                            clicked_event
                                .original_color
                                .adjust(&clicked_event.color_diff),
                        );
                    }
                }
                _ => (),
            }
        }

        // The handling of the mouse click is before the calculation of the layout.  This safe
        // based on the two assumptions:
        //
        // 1. The user does not resize and click at the same time.
        // 2. The user clicks on the events only when they're visible.
        let maybe_clicked_event: Option<EventDetails> = event_mouse_click.and_then(|mouse_click| {
            let MouseEventClick {
                event_kind,
                position,
            } = mouse_click;
            let rectangles: EventRectangles = self.calendar.state.obtain_events();
            let (is_long, rectangles): (_, _) = match event_kind {
                CalendarEventKind::Long => (true, rectangles.long),
                CalendarEventKind::Short => (false, rectangles.short),
            };

            find_clicked_event(&position, rectangles).and_then(|event: usize| {
                let table = self.calendar.state.get_event_table(is_long)?;
                let title = table.obtain_title(event as u32)?;
                let range = table.obtain_range(event as u32)?;
                let description = table.obtain_description(event as u32)?;
                Some(EventDetails {
                    title,
                    description,
                    event_kind,
                    // FIXME(alex): make a special type for the indexes of events.
                    index: event as u32,
                    range,
                })
            })
        });

        match maybe_clicked_event {
            Some(event_details) => {
                let form_field_content_registry: &RefCell<F::TextObjectRegistry> =
                    frontend.get_event_details_text_object_regirsty();
                let form_field_label_registry: &RefCell<F::TextTextureRegistry> =
                    frontend.get_event_details_field_label_regirsty();
                self.event_details_view = Activities::<F>::create_event_details_text_objects(
                    event_details,
                    &window_size,
                    &mut form_field_content_registry.borrow_mut(),
                    &mut form_field_label_registry.borrow_mut(),
                    Color::WHITE,
                )?
                .into();

                Ok(NewState {
                    activity: Activity::EventView,
                    render_data: RenderData::EventView(EventViewRenderData {
                        offsets: self
                            .event_details_view
                            .as_ref()
                            .map(|view| view.offsets.as_ref())
                            .unwrap_or(&[]),
                        highlight: Vec::new(),
                        frontend: &*frontend,
                        textbox: self
                            .event_details_view
                            .as_ref()
                            .and_then(|v| v.description_textbox.as_ref())
                            .map(|tb| &tb.border_rect),
                        cursor: self
                            .event_details_view
                            .as_ref()
                            .and_then(|v| v.description_textbox.as_ref())
                            .and_then(|tb| tb.cursor_rect.as_ref()),
                    }),
                })
            }
            None => {
                // If the user switches the week, the events for the week are requested from Khal.
                if self.calendar.is_week_switched {
                    self.calendar.update_week_data(frontend)?;
                    let cell_width = DUMB_CELL_WIDTH;
                    frontend.get_dates_text_registry().clear();
                    App::create_dates_text_objects(
                        frontend,
                        cell_width,
                        &self.calendar.week_start,
                    )?;
                }

                // The events has been delivered, get ready to render them!
                self.calendar.get_rendering(frontend);

                let long_event_clash_size: calendar::Lane =
                    self.calendar.state.get_long_event_clash_size();
                // The viewport through which we watch the surface with the short events.
                let short_event_viewport: Rect = {
                    let long_event_surface_height = View::compute_long_event_surface_height(
                        self.ui.title_font_height,
                        long_event_clash_size,
                    );
                    ShortEventViewport::new(
                        &self.ui.event_offset,
                        &window_size,
                        long_event_surface_height,
                    )
                    .into_rect()
                };

                // Create the view with the short events.
                let long_event_surface_height = View::compute_long_event_surface_height(
                    self.ui.title_font_height,
                    long_event_clash_size,
                );

                let view: calendar::ui::View = Self::create_view(
                    &self.ui.event_offset,
                    &self.ui.adjustment,
                    &window_size,
                    long_event_surface_height,
                );

                let hours_viewport = Rect {
                    y: short_event_viewport.y,
                    x: 10,
                    w: self.ui.event_offset.x as i32,
                    h: window_size.y,
                };

                // reposition the hours text objects based on
                // - short event cell height.
                // - vertical offset of the surface with the short events.
                Self::reposition_hours_text_objects(frontend, hours_viewport.w as f32, &view);
                // reposition the day names objects based on
                // - short event cell height and width.
                Self::reposition_days_text_objects(frontend, 35f32, &view);
                // reposition the dates text objects based on
                // - short event cell height and width.
                Self::reposition_dates_text_objects(frontend, 10f32, &view);

                // The heaviest part of creation of the render data.  The computation is based on the data
                // received from Khal.  Given the data, the underlying program does the following:
                // 1. Computes the positions and the sizes of the rectangles for the events (both long and
                //    short).  This takes into account the clashes of the events.
                // 2. Based on the sizes of the rectangles it renders the texts.
                let top_panel_height = View::compute_long_event_surface_height(
                    self.ui.title_font_height,
                    long_event_clash_size,
                );
                self.calendar.get_ready(
                    &view,
                    top_panel_height,
                    frontend,
                    &self.ui.event_title_offset,
                    &self.ui.event_offset,
                )?;

                let rectangles: EventRectangles = self.calendar.state.obtain_events();
                let horizontal_offset = self.ui.event_offset.x as i32;
                let dates_viewport = Rect {
                    x: horizontal_offset,
                    y: 0,
                    w: window_size.x - horizontal_offset,
                    h: 200,
                };

                let render_data = WeekViewRenderData {
                    view,
                    long_event_rectangles: rectangles.long,
                    hours_viewport,
                    dates_viewport,
                    short_event_rectangles: rectangles.short,
                    event_viewport: short_event_viewport,
                    frontend,
                };

                Ok(NewState {
                    activity: Activity::WeekView,
                    render_data: RenderData::WeekView(render_data),
                })
            }
        }
    }

    pub fn get_root_activity(&self) -> Activity {
        Activity::WeekView
    }

    // main function
    pub fn create_render_data<'wdrect, 'frontend>(
        &'wdrect mut self,
        activity: Activity,
        frontend: &'frontend mut F,
        window_size: Point,
        events: impl IntoIterator<Item = Action>,
    ) -> Result<NewState<'wdrect, 'frontend, F>, F::Error> {
        match activity {
            Activity::WeekView => self.create_week_view_render_data(frontend, window_size, events),
            Activity::EventView => {
                self.create_event_view_render_data(frontend, window_size, events)
            }
        }
    }

    fn create_event_view_render_data<'wdrect, 'frontend>(
        &'wdrect mut self,
        frontend: &'frontend mut F,
        window_size: Point,
        events: impl IntoIterator<Item = Action>,
    ) -> Result<NewState<'wdrect, 'frontend, F>, F::Error> {
        for event in events {
            match event {
                // Scrolling a single line text which doesn't fit its field (a.k.a. viewport).
                Action::TextScroll { offset, x, y } => {
                    let offset = offset * TEXT_SCROLL_AMPLIFIER;
                    let registry = frontend.get_event_details_text_object_regirsty().borrow();
                    let positions = registry.get_positions();
                    if let Some((scrolled_text_index, position)) = positions
                        .iter()
                        .enumerate()
                        .find(|(_, pos)| pos.covers_point(&FPoint { x, y }))
                        && let Some(text) = registry.get(scrolled_text_index)
                    {
                        let mut new_offset = 0f32;
                        if let Some(view) = self.event_details_view.as_ref() {
                            // The offsets are as many as text objects.  Given that, the default
                            // value is not expected to be set.
                            let current_offset = view
                                .offsets
                                .get(scrolled_text_index)
                                .cloned()
                                .unwrap_or_default();
                            new_offset = current_offset;

                            // It's assumed that the scrollable text has only line.  A multiline
                            // text is supposed to be only in the description.
                            // FIXME(alex): add sanitazing of the single line texts.  Replace the
                            // line break character with something.  Alternatively, figure out a
                            // better handling of this case.
                            let text_rect: Option<FRect> = view
                                .texts
                                .get(scrolled_text_index)
                                .and_then(|text_string| {
                                    let text_engine = frontend.get_text_engine();
                                    text_engine
                                        .calculate_highlights(text, 0, text_string.len() as i32)
                                        .ok()
                                })
                                .and_then(|v| v.into_iter().next());

                            if let Some(text_rect) = text_rect.filter(|r| r.w > position.w) {
                                new_offset =
                                    (current_offset + offset).clamp(position.w - text_rect.w, 0f32);
                            }
                        }

                        if let Some(offset) = self
                            .event_details_view
                            .as_mut()
                            .and_then(|view| view.offsets.get_mut(scrolled_text_index))
                        {
                            *offset = new_offset;
                        }
                    }
                }
                Action::Escape => {
                    return self.create_week_view_render_data(
                        frontend,
                        window_size,
                        Vec::new().into_iter(),
                    );
                }
                Action::MouseMove {
                    x,
                    y,
                    pressed_button: _,
                } => {
                    let maybe_textbox = self
                        .event_details_view
                        .as_mut()
                        .and_then(|view| view.description_textbox.as_mut())
                        .filter(|textbox| textbox.is_highlighting);

                    // As long as the user hasn't released the mouse button, the highlighting is
                    // on.
                    if let Some(textbox) = maybe_textbox {
                        assert!(textbox.highlight_start != -1);
                        let registry = frontend.get_event_details_text_object_regirsty();
                        if let Some(text_object) = registry.borrow().get(DESCRIPTION_TEXT_INDEX) {
                            let text_engine = frontend.get_text_engine();
                            // The position is relative to the rectangle shaping of the text.
                            // Currently it's the border of it.
                            let relative_position = FPoint {
                                x: x - textbox.border_rect.x,
                                y: y - textbox.border_rect.y,
                            };

                            if let Ok(offset) =
                                text_engine.get_offset(text_object, &relative_position)
                            {
                                textbox.highlight_end = offset;
                            }
                        }
                    }
                }
                Action::MouseButtonUp { .. } => {
                    let maybe_textbox = self
                        .event_details_view
                        .as_mut()
                        .and_then(|view| view.description_textbox.as_mut());
                    if let Some(textbox) = maybe_textbox {
                        textbox.is_highlighting = false;
                    }
                }
                Action::MouseButtonDown {
                    position,
                    button: MouseButton::Left,
                } => {
                    // reset the the second marker and set the state
                    let maybe_textbox = self
                        .event_details_view
                        .as_mut()
                        .and_then(|view| view.description_textbox.as_mut());

                    if let Some(textbox) = maybe_textbox {
                        let text_engine = frontend.get_text_engine();
                        let registry = frontend.get_event_details_text_object_regirsty();
                        if textbox.border_rect.covers_point(&position) {
                            // FIXME(alex): The index should correspond the picked textbox when
                            // we have a few of them.
                            if let Some(text_object) = registry.borrow().get(DESCRIPTION_TEXT_INDEX)
                            {
                                textbox.is_highlighting = true;
                                textbox.highlight_end = -1;
                                // NOTE(alex): this might different if the text has some margin
                                // around itself.
                                let textrect = &textbox.border_rect;
                                let relative_position: FPoint =
                                    position.sub_fpoint(textrect.x, textrect.y);
                                if let Ok(offset) =
                                    text_engine.get_offset(text_object, &relative_position)
                                {
                                    textbox.highlight_start = offset;
                                }
                            }
                        }
                    };
                }
                Action::Yank => {
                    let maybe_textbox: Option<&Textbox> = self
                        .event_details_view
                        .as_ref()
                        .and_then(|view| view.description_textbox.as_ref())
                        .filter(|tb| tb.highlight_start != -1 && tb.highlight_end != -1);

                    if let Some(textbox) = maybe_textbox
                        && let Some(description) = self.get_selected_event_desription()
                    {
                        let start = textbox.highlight_start.min(textbox.highlight_end);
                        let end = textbox.highlight_start.max(textbox.highlight_end);
                        // NOTE(alex): end might be wrong. Prevent off by one error.
                        let copied_text: &str = &description[start as usize..end as usize];
                        frontend.set_clipboard(copied_text)?;
                    }
                }
                Action::WindowResize => {
                    let window_width = window_size.x as f32;
                    // FIXME(alex): store this offset somewhere and pass to the functions which
                    // creates the text objects in Activities::create_event_details_text_objects.
                    const OFFSET: f32 = 300.;
                    let text_width = window_width - OFFSET;
                    let registry: &RefCell<F::TextObjectRegistry> =
                        frontend.get_event_details_text_object_regirsty();

                    {
                        let maybe_flexible_fields: Option<&[u32]> = self
                            .event_details_view
                            .as_ref()
                            .map(|view| view.flexible_fields.as_ref());
                        if let Some(flexible_fields) = maybe_flexible_fields {
                            let mut regref = registry.borrow_mut();
                            let positions: &mut _ = regref.get_positions_mut();
                            for i in flexible_fields.iter() {
                                positions[*i as usize].w = text_width;
                            }
                        }
                    }

                    // The error is raised if the descrption does not exist or if SDL fails to set
                    // the wrapping
                    let _ = registry
                        .borrow_mut()
                        .set_wrap(DESCRIPTION_TEXT_INDEX as u32, text_width);
                    let maybe_textbox: Option<&mut Textbox> = self
                        .event_details_view
                        .as_mut()
                        .and_then(|view| view.description_textbox.as_mut());
                    if let Some(textbox) = maybe_textbox {
                        textbox.border_rect.w = text_width;
                    }

                    // NOT PLANNED
                    if let Some(offsets) =
                        self.event_details_view.as_mut().map(|v| v.offsets.as_mut())
                    {
                        offsets.fill(0f32);
                    }
                }
                _ => (),
            }
        }

        let maybe_textbox: Option<&Textbox> = self
            .event_details_view
            .as_mut()
            .and_then(|view| view.description_textbox.as_ref());
        let render_highlights: Vec<FRect> = maybe_textbox
            .filter(|tb| tb.highlight_start != -1 && tb.highlight_end != -1)
            .and_then(|textbox: &Textbox| {
                let registry = frontend.get_event_details_text_object_regirsty();
                let text_engine = frontend.get_text_engine();
                registry
                    .borrow()
                    .get(DESCRIPTION_TEXT_INDEX)
                    .and_then(|text_object| {
                        // normalizing for the case when the highlighting starts from right bottom
                        // to left top.
                        let start = textbox.highlight_start.min(textbox.highlight_end);
                        let end = textbox.highlight_start.max(textbox.highlight_end);
                        let len = end - start;
                        text_engine
                            .calculate_highlights(text_object, start, len)
                            .ok()
                    })
                    .map(|mut highlights: Vec<FRect>| {
                        // shift the rectangles of highlighting to the coordinates relative to
                        // the window.
                        for item in highlights.iter_mut() {
                            item.x += textbox.border_rect.x;
                            item.y += textbox.border_rect.y;
                        }
                        highlights
                    })
            })
            .unwrap_or_default();

        let maybe_textbox: Option<&mut Textbox> = self
            .event_details_view
            .as_mut()
            .and_then(|view| view.description_textbox.as_mut());
        if let Some(textbox) = maybe_textbox {
            let cursor: Option<i32> = match (textbox.highlight_start, textbox.highlight_end) {
                (-1, -1) => None,
                (x, -1) => Some(x),
                (_x, y) => Some(y),
            };

            if let Some(cursor) = cursor {
                let text_engine = frontend.get_text_engine();
                let registry = frontend.get_event_details_text_object_regirsty();
                let rect = registry
                    .borrow()
                    .get(DESCRIPTION_TEXT_INDEX)
                    .and_then(|descrption| {
                        text_engine.calculate_highlights(descrption, cursor, 1).ok()
                    });

                if let Some(mut cursor_rect) = rect.and_then(|r| r.into_iter().next()) {
                    cursor_rect =
                        cursor_rect.move_frect(textbox.border_rect.x, textbox.border_rect.y);
                    textbox.cursor_rect = Some(cursor_rect)
                }
            }
        }

        Ok(NewState {
            activity: Activity::EventView,
            render_data: RenderData::EventView(EventViewRenderData {
                // TODO(alex):
                // Add the zero offsets
                offsets: self
                    .event_details_view
                    .as_ref()
                    .map(|view| view.offsets.as_ref())
                    .unwrap(),
                frontend,
                highlight: render_highlights,
                textbox: self
                    .event_details_view
                    .as_ref()
                    .and_then(|v| v.description_textbox.as_ref())
                    .map(|tb| &tb.border_rect),
                cursor: self
                    .event_details_view
                    .as_ref()
                    .and_then(|v| v.description_textbox.as_ref())
                    .and_then(|tb| tb.cursor_rect.as_ref()),
            }),
        })
    }
}

struct Activities<F: Frontend> {
    _frontend: core::marker::PhantomData<F>,
}

#[inline]
fn format_date_time(date: &calendar::date::Date, time: &calendar::date::Time) -> String {
    format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        date.year, date.month, date.day, time.hour, time.minute
    )
}

impl<F: Frontend> Activities<F> {
    // Renders the text of the event details
    fn create_event_details_text_objects(
        details: EventDetails,
        window_size: &Point,
        event_details_text_object_regirsty: &mut F::TextObjectRegistry,
        event_details_field_label_regirsty: &mut F::TextTextureRegistry,
        label_color: Color,
    ) -> Result<EventDetailsView, F::Error> {
        event_details_text_object_regirsty.clear();
        let mut field_counter = 0;
        // Assuming that 10 is the maximum possible number of field for a calendar event.
        const MAX_FIELDS: usize = 10;
        let mut texts: Vec<Box<str>> = Vec::with_capacity(MAX_FIELDS);
        let mut flexible_fields: [u32; MAX_FIELDS] = [0; MAX_FIELDS];
        let mut flexible_fields_cursor: usize = 0;
        let mut push_flexible_field = |value| {
            flexible_fields[flexible_fields_cursor] = value;
            flexible_fields_cursor += 1;
        };

        // FIXME(alex): this should be based on the font line height
        let one_line_height = 30f32;
        let top_offset = 100f32;
        let mut vertical_offset = top_offset;
        event_details_field_label_regirsty.create(
            captions::event_details_view::TITLE,
            label_color,
            FRect {
                x: 100.0,
                y: vertical_offset,
                w: window_size.x as f32 - 200.0,
                h: one_line_height,
            },
        )?;

        vertical_offset += one_line_height;
        // FIXME(alex): a long title is cropped
        event_details_text_object_regirsty.create(
            details.title,
            FRect {
                x: 150.0,
                y: vertical_offset,
                w: window_size.x as f32 - 200.0,
                h: one_line_height,
            },
        )?;
        texts.push(Box::from(details.title));

        push_flexible_field(field_counter);
        field_counter += 1;

        vertical_offset += one_line_height * 2.;
        event_details_field_label_regirsty.create(
            captions::event_details_view::FROM,
            label_color,
            FRect {
                x: 100.0,
                y: vertical_offset,
                w: window_size.x as f32 - 200.0,
                h: one_line_height,
            },
        )?;

        vertical_offset += one_line_height;
        // FIXME(alex): the width of the field should be based on the size of the font.
        const DATE_TIME_FIELD_WIDTH: f32 = 220.0;
        let start = format_date_time(&details.range.start_date, &details.range.start_time);
        texts.push(Box::from(start));
        event_details_text_object_regirsty.create(
            // FIXME(alex): the date should be formatted according the locale chosen by the user
            texts.last().unwrap().as_ref(),
            FRect {
                x: 150.0,
                y: vertical_offset,
                w: DATE_TIME_FIELD_WIDTH,
                h: one_line_height,
            },
        )?;

        field_counter += 1;

        vertical_offset += one_line_height;
        event_details_field_label_regirsty.create(
            captions::event_details_view::UNTIL,
            label_color,
            FRect {
                x: 100.0,
                y: vertical_offset,
                w: window_size.x as f32 - 200.0,
                h: one_line_height,
            },
        )?;

        vertical_offset += one_line_height;
        let until = format_date_time(&details.range.end_date, &details.range.end_time);
        texts.push(Box::from(until));
        event_details_text_object_regirsty.create(
            // FIXME(alex): the date should be formatted according the locale chosen by the user
            texts.last().unwrap().as_ref(),
            FRect {
                x: 150.0,
                y: vertical_offset,
                w: DATE_TIME_FIELD_WIDTH,
                h: one_line_height,
            },
        )?;

        field_counter += 1;

        let description_textbox: Option<Textbox> = if !details.description.is_empty() {
            vertical_offset += 2f32 * one_line_height;
            event_details_field_label_regirsty.create(
                captions::event_details_view::DESCRIPTION,
                label_color,
                FRect {
                    x: 100.0,
                    y: vertical_offset,
                    w: window_size.x as f32 - 200.0,
                    h: one_line_height,
                },
            )?;

            vertical_offset += one_line_height;
            // FIXME(alex): a long description is cropped
            let border_rect = FRect {
                x: 150.0,
                y: vertical_offset,
                w: window_size.x as f32 - 200.0,
                h: window_size.y as f32 - vertical_offset,
            };
            event_details_text_object_regirsty.create(details.description, border_rect)?;
            event_details_text_object_regirsty
                .set_wrap(DESCRIPTION_TEXT_INDEX as u32, border_rect.w)?;
            texts.push(Box::from(details.description));

            push_flexible_field(field_counter);
            field_counter += 1;
            Some(Textbox::new(border_rect))
        } else {
            None
        };

        Ok(EventDetailsView {
            description_textbox,
            event_index: details.index,
            event_kind: details.event_kind,
            flexible_fields: Box::from(&flexible_fields[..flexible_fields_cursor]),
            texts: texts.into_boxed_slice(),
            offsets: (0..field_counter).map(|_| 0f32).collect(),
        })
    }
}

struct EventDetails<'event> {
    title: &'event str,
    description: &'event str,
    index: u32,
    event_kind: CalendarEventKind,
    range: &'event calendar::EventRange,
}

// If mouse_position is within the surface of the long events or the short events then
// [`MouseEventClick`] is created.
fn try_register_mouse_click(
    mouse_position: FPoint,
    long_event_surface: &LongEventSurface,
    window_size: &Point,
) -> Option<MouseEventClick> {
    let short_event_viewport =
        ShortEventViewport::from_long_event_surface(long_event_surface, window_size);

    let is_long_event_click = {
        let size = long_event_surface.size;
        is_fpoint_between_points(
            mouse_position,
            long_event_surface.offset,
            long_event_surface.offset.add_fpoint(size.x, size.y),
        )
    };

    let is_short_event_click = {
        let size = short_event_viewport.size;
        is_fpoint_between_points(
            mouse_position,
            short_event_viewport.offset,
            short_event_viewport.offset.add_fpoint(size.x, size.y),
        )
    };

    if is_long_event_click {
        Some(MouseEventClick {
            event_kind: CalendarEventKind::Long,
            position: mouse_position,
        })
    } else if is_short_event_click {
        let offset = short_event_viewport.offset;
        let position = mouse_position.sub_fpoint(offset.x, offset.y);
        Some(MouseEventClick {
            event_kind: CalendarEventKind::Short,
            position,
        })
    } else {
        None
    }
}

pub struct NewState<'rect, 'frontend, F> {
    pub activity: Activity,
    pub render_data: RenderData<'rect, 'frontend, F>,
}

fn find_clicked_event(
    position: &FPoint,
    rectangles: &calendar::render::Rectangles,
) -> Option<usize> {
    rectangles.iter().position(|rect| {
        let left_top = rect.at;
        let bottom_right = rect.at.add_fpoint(rect.size.x, rect.size.y);
        is_fpoint_between_points(position, left_top, bottom_right)
    })
}

pub enum Activity {
    WeekView,
    EventView,
}

enum CalendarEventKind {
    // the position is relative to the long event surface
    Long,
    // the position is relative to the short event viewport
    Short,
}

impl CalendarEventKind {
    fn is_long(&self) -> bool {
        match self {
            CalendarEventKind::Long => true,
            CalendarEventKind::Short => false,
        }
    }
}

struct MouseEventClick {
    event_kind: CalendarEventKind,
    position: FPoint,
}

#[derive(Clone, Copy)]
struct NonNegativeF32(f32);

impl NonNegativeF32 {
    fn clip_from(value: f32) -> Self {
        Self(0f32.max(value))
    }
}

impl From<NonNegativeF32> for f32 {
    fn from(value: NonNegativeF32) -> Self {
        value.0
    }
}

/// When the surface with the short events is scaled, technically it means that only its size
/// changes.  Given that, the events slip away from under the mouse.  Therefore, the vertical
/// offset of the surface is to be adjusted.
///
/// # Arguments
///
/// `mouse` is the mouse cursor position
///
/// `new_vertical_scale` the amout of pixels to add to the height of the surface with the short
/// events.
///
/// `short_event_viewport_offset` the position of the viewport of the surface with the short
/// events.  The position is relative to the window.
///
/// `short_event_viewport_size` the size of the viewport mentioned above.
///
/// `current_adjustment` the adjustment of the surface within its viewport.
fn compute_cursor_adjustment(
    mouse: &FPoint,
    new_vertical_scale: NonNegativeF32,
    short_event_viewport_offset: &FPoint,
    short_event_viewport_size: &FPoint,
    current_adjustment: &SurfaceAdjustment,
) -> f32 {
    // The size of the surface with the short events _after_ the scaling is applied.
    let scaled_short_event_surface = calendar::ui::compute_event_surface(
        short_event_viewport_size,
        f32::from(new_vertical_scale),
        current_adjustment.vertical_offset,
    );

    let is_within = is_fpoint_between_points(
        mouse,
        short_event_viewport_offset,
        short_event_viewport_offset
            .add_fpoint(short_event_viewport_size.x, short_event_viewport_size.y),
    );
    // if the mouse cursor is within the viewport
    if is_within {
        // The size of the surface with the short events _before_ the scaling is applied.
        let current_short_event_surface = calendar::ui::compute_event_surface(
            short_event_viewport_size,
            current_adjustment.vertical_scale,
            current_adjustment.vertical_offset,
        );

        let current_abs_mouse: f32 =
            mouse.y - short_event_viewport_offset.y - current_short_event_surface.y;
        // Given the height of the surface 100px and the position of the cursor 10px, the `old_rel_mouse` is 10% (0.1).
        let old_rel_mouse: f32 = current_abs_mouse / current_short_event_surface.h;

        // Given the `old_rel_mouse` is 10% and the height of the surface after the scaling is
        // 120px, the mouse cursor position is to stay at 12px to be above the same event.
        let new_abs_position = scaled_short_event_surface.h * old_rel_mouse;
        // Given the values from above, the mouse cursor position is to be changed by 2px.
        // Therefore, we return -2px as the difference for vertial offset of the surface.
        new_abs_position - current_abs_mouse
    } else {
        0.
    }
}

/// Sensibly changes the scale of the short event surface.  When the surface is scaled out (it
/// gets smaller), a gap appears between the bottom of the surface and the bottom of its viewport.
/// Given that, the vertical offset of the surface is to be adjusted to prevent the gap.
fn scale_short_events(
    current: &SurfaceAdjustment,
    short_event_viewport_size: &FPoint,
    new_vertical_scale: NonNegativeF32,
) -> SurfaceAdjustment {
    // The size of the surface with the short events _after_ the scaling is applied.
    let scaled_short_event_surface = calendar::ui::compute_event_surface(
        short_event_viewport_size,
        f32::from(new_vertical_scale),
        current.vertical_offset,
    );

    let bottom = short_event_viewport_size.y;
    // This value can be smaller than the bottom edge of the viewport.
    let scaled_short_events_surface_bottom =
        scaled_short_event_surface.y + scaled_short_event_surface.h;
    let bottom_gap = bottom - scaled_short_events_surface_bottom;
    SurfaceAdjustment {
        vertical_offset: current.vertical_offset + bottom_gap.max(0f32),
        vertical_scale: f32::from(new_vertical_scale),
    }
}

pub enum Action {
    // FIXME(alex): replace this non-sense with key events. Requires to figure out how to handle
    // the modifying keys.  Think about how to apply Rust enums for it.
    Yank,
    Escape,

    WindowResize,
    SubtractWeek,
    AddWeek,
    // NOTE(alex): the following actions depends on the layout of the window.  This causes a quite
    // a couple of questions:
    //
    // 1. Should it be two kinds of Action?  Something like layout dependent and layout independent
    //    action?
    // 2. If an action causes layout change should it be within the frame in which the action
    //    handled or on the following?
    // 3. Probably, the entire layout is not needed for these events.  Given that, only the
    //    required can be calculated and the new layout is calculated based on the action
    // TODO(alex):
    // Add the mouse position to the event
    TextScroll {
        offset: f32,
        x: f32,
        y: f32,
    },
    Scroll {
        offset: f32,
        #[allow(unused)]
        x: f32,
        #[allow(unused)]
        y: f32,
    },
    Zoom(f32),
    MouseMove {
        x: f32,
        y: f32,
        pressed_button: Option<MouseButton>,
    },
    MouseButtonUp {
        position: FPoint,
    },
    MouseButtonDown {
        position: FPoint,
        button: MouseButton,
    },
}

pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forth,
}

fn create_long_events<'a, TTC: TextTextureRegistry>(
    event_data: &calendar::EventTable,
    week_start: &calendar::date::Date,
    mut registration: EventTitleRegistration<'a, TTC>,
    event_offset: &FPoint,
    cell_width: f32,
    top_panel_height: f32,
) -> Result<calendar::render::Rectangles, TTC::Error> {
    let replacement = calendar::ui::create_long_event_rectangles(
        event_offset,
        event_data,
        week_start,
        cell_width,
        top_panel_height,
    );

    registration.text_registry.clear();
    register_event_titles(&mut registration, &event_data.titles, &replacement).map(|_| replacement)
}

fn create_short_events<'a, TTC: TextTextureRegistry>(
    event_data: &calendar::EventTable,
    week_start: &calendar::date::Date,
    mut registration: EventTitleRegistration<'a, TTC>,
    view: &View,
) -> Result<calendar::render::Rectangles, TTC::Error> {
    let new_rectangles = calendar::ui::create_short_event_rectangles(
        &view.short_event_surface,
        event_data,
        week_start,
    );

    registration.text_registry.clear();
    register_event_titles(&mut registration, &event_data.titles, &new_rectangles)
        .map(|_| new_rectangles)
}

pub trait GetLongEventTextRegistry {
    type Registry;
    fn get_long_event_text_registry(&mut self) -> &mut Self::Registry;
}

pub trait GetShortEventTextRegistry {
    type Registry;
    fn get_short_event_text_registry(&mut self) -> &mut Self::Registry;
}

pub trait TextEngine {
    type TextObject;
    type Error;

    fn get_offset(
        &self,
        text_object: &Self::TextObject,
        position: &sdl3_sys::SDL_FPoint,
    ) -> Result<i32, Self::Error>;

    fn calculate_highlights(
        &self,
        text_object: &Self::TextObject,
        start: i32,
        len: i32,
    ) -> Result<Vec<FRect>, Self::Error>;
}

/// The trait provides the platform dependant functionality.  The main purpose of the abstraction
/// is provide the way to test the core.
pub trait Frontend:
    GetLongEventTextRegistry<Registry = Self::TextTextureRegistry>
    + GetShortEventTextRegistry<Registry = Self::TextTextureRegistry>
{
    type TextObject;
    type Error;
    type TextTextureRegistry: TextTextureRegistry<Error = Self::Error>;
    type TextObjectRegistry: TextObjectRegistry<Error = Self::Error, TextObject = Self::TextObject>;
    type AgendaSource: AgendaSource<Error = Self::Error>;
    type TextEngine: TextEngine<Error = Self::Error, TextObject = Self::TextObject>;

    fn get_hours_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_days_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_dates_text_registry(&mut self) -> &mut Self::TextTextureRegistry;

    fn get_event_details_field_label_regirsty(&self) -> &RefCell<Self::TextTextureRegistry>;

    fn get_event_details_text_object_regirsty(&self) -> &RefCell<Self::TextObjectRegistry>;
    //fn get_event_details_text_object_regirsty_mut(&mut self) -> &mut Self::TextObjectRegistry;
    fn get_text_engine(&self) -> &Self::TextEngine;

    fn get_current_week_start(&self) -> Result<calendar::date::Date, Self::Error>;
    fn set_clipboard(&self, text: impl Into<Vec<u8>>) -> Result<(), Self::Error>;

    fn agenda_source(&self) -> &Self::AgendaSource;
}

/// The trait to fetch the data for the calendar.
///
/// The data for the calendar is fetched from a "slow" source.  Currently it's conceived to fetch
/// the data from an external program.  As a program takes time to provide the data, the trait is
/// designed to fetch the data asynchronously.
///
/// Happy path scenario:
/// The process runs with method [`AgendaSource::request`] which returns a handle to be polled by
/// the method [`AgendaSource::is_ready`].  When the data is ready it's fetched with method
/// [`AgendaSource::fetch`].
///
/// The process can be canceled with the method [`AgendaSource::cancel`].
///
/// The implementation is expected  to store the data behind the provided handles.  The caller
/// decides when to free the data.  It calls the method [`AgendaSource::free`] when the data is no
/// longer needed.
pub trait AgendaSource {
    type RequestHandle;
    type Error;

    fn request(
        &self,
        week_start: &calendar::date::Date,
    ) -> Result<Self::RequestHandle, Self::Error>;
    fn cancel(&self, handle: &Self::RequestHandle);
    fn is_ready(&self, handle: &Self::RequestHandle) -> bool;
    fn free(&self, handle: Self::RequestHandle);
    fn fetch(
        &self,
        handle: &Self::RequestHandle,
        week_start: &calendar::date::Date,
    ) -> calendar::obtain::WeekScheduleWithLanes;
}

/// Stores textures of the text objects.
pub trait TextTextureRegistry {
    type Error;

    /// The method updates the destination rectangles of the textures of the text objects which
    /// were created by [`Self::create`].  The iterator returns as much items as the number of the
    /// created text objects.
    fn update_positions(&mut self, values: impl Iterator<Item = FRect>);

    fn clear(&mut self);

    /// Creates a text object from `text`.  The text object is stored within the registry.
    fn create(
        &mut self,
        text: impl Into<Vec<u8>>,
        color: Color,
        position: FRect,
    ) -> Result<(), Self::Error>;
}

pub trait TextObjectRegistry {
    type Error;
    type TextObject;

    fn clear(&mut self);
    fn get(&self, index: usize) -> Option<&Self::TextObject>;

    /// Creates a text object from `text`.  The text object is stored within the registry.
    fn create(&mut self, text: impl Into<Vec<u8>>, position: FRect) -> Result<(), Self::Error>;

    fn get_positions_mut(&mut self) -> &mut [FRect];
    // NOT PLANNED
    fn get_positions(&self) -> &[sdl3_sys::SDL_FRect];

    /// Sets the text wrap length
    fn set_wrap(&mut self, index: u32, width: f32) -> Result<(), Self::Error>;
}

struct EventTitleRegistration<'a, TTC: TextTextureRegistry> {
    event_title_offset: &'a FPoint,
    text_registry: &'a mut TTC,
}

fn register_event_titles<'a, Str, TTC: TextTextureRegistry>(
    registration: &mut EventTitleRegistration<'a, TTC>,
    titles: &[Str],
    rectangles: &[calendar::render::Rectangle],
) -> Result<(), TTC::Error>
where
    Str: AsRef<str>,
{
    let offset = registration.event_title_offset;
    let text_registry: &mut _ = registration.text_registry;
    assert_eq!(titles.len(), rectangles.len());
    for item in titles.iter().zip(rectangles.iter()) {
        let (title, rectangle): (&Str, &calendar::render::Rectangle) = item;
        let dstrect = FRect {
            x: rectangle.at.x + offset.x,
            y: rectangle.at.y + offset.y,
            w: rectangle.size.x - offset.x * 2f32,
            h: rectangle.size.y - offset.y * 2f32,
        };

        text_registry.create(title.as_ref(), Color::BLACK, dstrect)?;
    }
    Ok(())
}
