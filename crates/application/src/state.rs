use crate::config;

use crate::render::RenderData;

use calendar::{
    date::DateStream,
    ui::{SurfaceAdjustment, View},
};

use sdl3_sys::{SDL_FPoint as FPoint, SDL_FRect as FRect, SDL_Point as Point, SDL_Rect as Rect};
use sdlext::Color;

pub struct Calendar<F: Frontend> {
    _frontend: std::marker::PhantomData<F>,
    pub week_start: calendar::date::Date,
    pub is_week_switched: bool,
    state: CalendarState<<F::AgendaSource as AgendaSource>::RequestHandle>,
}

enum CalendarState<Handle> {
    Loading {
        agenda_source_handle: Handle,
    },
    Ready {
        week_data: WeekData,
        long_event_clash_size: calendar::Lane,
        short_event_rectangles_opt: calendar::render::Rectangles,
        long_event_rectangles_opt: calendar::render::Rectangles,
    },
    Rendering {
        week_data: WeekData,
        long_event_clash_size: calendar::Lane,
    },
}

impl<H> CalendarState<H> {
    fn obtain_events<'a>(&'a self) -> EventRectangles<'a> {
        match self {
            Self::Loading { .. } => EventRectangles {
                long: &NO_RECT,
                short: &NO_RECT,
            },
            Self::Ready {
                short_event_rectangles_opt,
                long_event_rectangles_opt,
                ..
            } => EventRectangles {
                long: long_event_rectangles_opt,
                short: short_event_rectangles_opt,
            },
            Self::Rendering { .. } => {
                unreachable!("unexpected state of the calendar")
            }
        }
    }

    /// It provides a memory-safe way to switch the state.  The function creates an uninitialized
    /// state to replace the current one.  Then it tries to switch to the next state provided by
    /// the function `update`.  The function must return any valid state and an error if any has
    /// occurred.
    fn switch<E>(&mut self, mut update: impl FnMut(Self) -> (Self, Option<E>)) -> Result<(), E> {
        // SAFETY: the bald_state is never read until the function finishes.
        let bald_state: CalendarState<_> = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        let current_state = std::mem::replace(self, bald_state);
        let (new_state, error) = update(current_state);
        *self = new_state;
        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn switch_infallible(&mut self, update: impl Fn(Self) -> Self) {
        // SAFETY: the bald_state is never read until the function finishes.
        // FIXME(alex): the state is 200 bytes long.
        let bald_state: CalendarState<_> = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        let current_state = std::mem::replace(self, bald_state);
        let new_state = update(current_state);
        *self = new_state;
    }

    /// a shortcut to switch to the [`CalendarState<H>::Loading`]
    fn loading<E>(agenda_source_handle: H, e: Option<E>) -> (Self, Option<E>) {
        (
            Self::Loading {
                agenda_source_handle,
            },
            e,
        )
    }
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

    // transition from rendering to ready
    fn get_ready<'ttc>(
        &mut self,
        view: &View,
        long_event_text_registry: &'ttc mut F::TextTextureRegistry,
        short_event_text_registry: &'ttc mut F::TextTextureRegistry,
        event_offset: &FPoint,
    ) -> Result<(), F::Error> {
        use CalendarState::*;
        self.state.switch(|current_state| match current_state {
            Loading { .. } | Ready { .. } => (current_state, None),
            Rendering {
                week_data,
                long_event_clash_size,
            } => {
                let short_event_rectangles_opt = create_short_events(
                    &week_data.agenda.short,
                    &self.week_start,
                    short_event_text_registry,
                    view,
                );

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

                let top_panel_heigth: f32 = view.calculate_top_panel_height();
                let long_event_rectangles_opt = create_long_events(
                    &week_data.agenda.long,
                    &self.week_start,
                    long_event_text_registry,
                    event_offset,
                    view.cell_width,
                    top_panel_heigth,
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
}

static NO_RECT: calendar::render::Rectangles = Vec::new();
struct EventRectangles<'rect> {
    long: &'rect calendar::render::Rectangles,
    short: &'rect calendar::render::Rectangles,
}

pub struct UserInterface {
    pub adjustment: SurfaceAdjustment,
    pub title_font_height: std::ffi::c_int,
    /// From where the all of the events are drawn.
    pub event_offset: FPoint,
    pub mouse_position: FPoint,
}

impl UserInterface {
    fn new(
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
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
        let short_event_viewport_offset = {
            FPoint {
                x: self.event_offset.x,
                y: self.event_offset.y + long_event_surface_height,
            }
        };

        let short_event_viewport_size = FPoint {
            x: window_size.x as f32 - short_event_viewport_offset.x,
            y: window_size.y as f32 - short_event_viewport_offset.y,
        };

        let current_adjustment = &self.adjustment;
        let new_vertical_scale =
            NonNegativeF32::clip_from(current_adjustment.vertical_scale + scale_value);
        let diff: f32 = compute_cursor_adjustment(
            &self.mouse_position,
            new_vertical_scale,
            &short_event_viewport_offset,
            &short_event_viewport_size,
            current_adjustment,
        );

        let mouse_adjustment = SurfaceAdjustment {
            vertical_scale: current_adjustment.vertical_scale,
            vertical_offset: current_adjustment.vertical_offset - diff,
        };

        scale_short_events(
            &mouse_adjustment,
            &short_event_viewport_size,
            new_vertical_scale,
        )
    }
}

fn calculate_viewport(
    event_offset: &FPoint,
    window_size: &Point,
    long_event_surface_height: f32,
) -> Rect {
    let yoffset = event_offset.y + long_event_surface_height;
    Rect {
        x: event_offset.x as i32,
        y: yoffset as i32,
        w: window_size.x - event_offset.x as i32,
        h: window_size.y - yoffset as i32,
    }
}
// The state of the application
pub struct App<F: Frontend> {
    pub calendar: Calendar<F>,
    pub ui: UserInterface,
}

const DUMB_CELL_WIDTH: f32 = 130f32;

impl<F: Frontend> App<F> {
    fn get_clash_size(calendar: &Calendar<F>) -> calendar::Lane {
        match calendar.state {
            CalendarState::Loading { .. } => 0,
            CalendarState::Ready {
                long_event_clash_size,
                ..
            }
            | CalendarState::Rendering {
                long_event_clash_size,
                ..
            } => long_event_clash_size,
        }
    }

    pub fn new(
        frontend: &mut F,
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
        mouse_position: FPoint,
    ) -> Result<Self, F::Error> {
        let ui = UserInterface::new(title_font_height, event_offset, mouse_position);
        let calendar = Calendar::new(frontend)?;
        App::create_hours_text_objects(frontend, ui.event_offset.x)?;

        let cell_width = DUMB_CELL_WIDTH; // FIXME: the value must be calculated
        App::create_days_text_objects(frontend, cell_width)?;

        App::create_dates_text_objects(frontend, cell_width, &calendar.week_start)?;
        Ok(Self { calendar, ui })
    }

    #[inline]
    fn viewport_size(
        event_offset: &FPoint,
        window_size: &Point,
        long_event_surface_height: f32,
    ) -> FPoint {
        let yoffset = event_offset.y + long_event_surface_height;
        FPoint {
            x: window_size.x as f32 - event_offset.x,
            y: window_size.y as f32 - yoffset,
        }
    }

    fn create_view(&mut self, window_size: &Point) -> View {
        let clash_size: u8 = Self::get_clash_size(&self.calendar);
        let long_event_surface_height =
            View::compute_top_panel_height(self.ui.title_font_height, clash_size);
        View::new(
            Self::viewport_size(
                &self.ui.event_offset,
                window_size,
                long_event_surface_height,
            ),
            &self.ui.adjustment,
            self.ui.title_font_height,
            clash_size,
        )
    }

    fn reposition_hours_text_objects(&self, frontend: &mut F, width: f32, view: &View) {
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

    fn reposition_days_text_objects(&self, frontend: &mut F, offset: f32, view: &View) {
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

    fn reposition_dates_text_objects(&self, frontend: &mut F, offset: f32, view: &View) {
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

    pub fn create_render_data<'wdrect, 'ttc>(
        &'wdrect mut self,
        frontend: &'ttc mut F,
        window_size: Point,
        long_event_text_registry: &'ttc mut F::TextTextureRegistry,
        short_event_text_registry: &'ttc mut F::TextTextureRegistry,
        events: impl IntoIterator<Item = Action>,
    ) -> Result<RenderData<'wdrect, 'ttc, F::TextTextureRegistry, F>, F::Error> {
        for event in events {
            use Action::*;
            match event {
                WindowResize => self.calendar.request_render(),
                Scroll(value) => {
                    self.ui.add_adjustment(value);
                    self.calendar.request_render();
                }
                Zoom(value) => {
                    let long_event_surface_height: f32 = View::compute_top_panel_height(
                        self.ui.title_font_height,
                        Self::get_clash_size(&self.calendar),
                    );
                    self.ui.adjustment = self.ui.compute_short_event_surface_adjustment(
                        long_event_surface_height,
                        value,
                        &window_size,
                    );
                    self.calendar.request_render();
                }
                MouseMove { x, y } => {
                    self.ui.mouse_position.x = x;
                    self.ui.mouse_position.y = y;
                }
                SubtractWeek => self.calendar.subtract_week(),
                AddWeek => self.calendar.add_week(),
            }
        }

        if self.calendar.is_week_switched {
            self.calendar.update_week_data(frontend)?;
            let cell_width = DUMB_CELL_WIDTH;
            frontend.get_dates_text_registry().clear();
            App::create_dates_text_objects(frontend, cell_width, &self.calendar.week_start)?;
        }

        let bald_state: CalendarState<_> = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        let state = std::mem::replace(&mut self.calendar.state, bald_state);
        let new_state: &mut _ = &mut self.calendar.state;
        *new_state = match state {
            CalendarState::Loading {
                agenda_source_handle,
            } => {
                let src = frontend.agenda_source();
                if src.is_ready(&agenda_source_handle) {
                    let agenda: calendar::obtain::WeekScheduleWithLanes =
                        src.fetch(&agenda_source_handle, &self.calendar.week_start);
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
        };

        let long_event_surface_height = View::compute_top_panel_height(
            self.ui.title_font_height,
            Self::get_clash_size(&self.calendar),
        );

        let short_event_viewport: Rect = calculate_viewport(
            &self.ui.event_offset,
            &window_size,
            long_event_surface_height,
        );
        let view: calendar::ui::View = self.create_view(&window_size);
        let hours_viewport: Rect = {
            let y = short_event_viewport.y as i32;
            Rect {
                y,
                x: 10,
                w: self.ui.event_offset.x as i32,
                h: window_size.y,
            }
        };

        self.reposition_hours_text_objects(frontend, hours_viewport.w as f32, &view);
        let horizontal_offset = self.ui.event_offset.x as i32;
        let dates_viewport = Rect {
            x: horizontal_offset,
            y: 0,
            w: window_size.x - horizontal_offset,
            h: 200,
        };

        self.reposition_days_text_objects(frontend, 35f32, &view);
        self.reposition_dates_text_objects(frontend, 10f32, &view);
        self.calendar.get_ready(
            &view,
            long_event_text_registry,
            short_event_text_registry,
            &self.ui.event_offset,
        )?;
        let rectangles: EventRectangles = self.calendar.state.obtain_events();
        Ok(RenderData {
            view,
            long_event_rectangles: rectangles.long,
            hours_viewport,
            dates_viewport,
            short_event_rectangles: rectangles.short,
            long_event_text_registry,
            short_event_text_registry,
            event_viewport: short_event_viewport,
            frontend,
        })
    }
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

    let yrange = short_event_viewport_offset.y..short_event_viewport_offset.y + short_event_viewport_size.y;
    let xrange = short_event_viewport_offset.x..short_event_viewport_offset.x + short_event_viewport_size.x;
    // if the mouse cursor is within the viewport
    if yrange.contains(&mouse.y) && xrange.contains(&mouse.x) {
        // The size of the surface with the short events _before_ the scaling is applied.
        let current_short_event_surface = calendar::ui::compute_event_surface(
            short_event_viewport_size,
            current_adjustment.vertical_scale,
            current_adjustment.vertical_offset,
        );

        let current_abs_mouse: f32 = mouse.y - short_event_viewport_offset.y - current_short_event_surface.y;
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
    WindowResize,
    Scroll(f32),
    Zoom(f32),
    MouseMove { x: f32, y: f32 },
    SubtractWeek,
    AddWeek,
}

pub fn create_long_events<TTC: TextTextureRegistry>(
    event_data: &calendar::EventData,
    week_start: &calendar::date::Date,
    text_registry: &mut TTC,
    offset: &FPoint,
    cell_width: f32,
    top_panel_height: f32,
) -> Result<calendar::render::Rectangles, TTC::Error> {
    let replacement = calendar::ui::create_long_event_rectangles(
        offset,
        event_data,
        week_start,
        cell_width,
        top_panel_height,
    );

    text_registry.clear();
    register_event_titles(text_registry, &event_data.titles, &replacement).map(|_| replacement)
}

fn create_short_events<TTC: TextTextureRegistry>(
    event_data: &calendar::EventData,
    week_start: &calendar::date::Date,
    text_registry: &mut TTC,
    view: &View,
) -> Result<calendar::render::Rectangles, TTC::Error> {
    let new_rectangles = calendar::ui::create_short_event_rectangles(
        &view.short_event_surface,
        event_data,
        week_start,
    );

    text_registry.clear();
    register_event_titles(text_registry, &event_data.titles, &new_rectangles)
        .map(|_| new_rectangles)
}

/// The trait provides the platform dependant functionality.  The main purpose of the abstraction
/// is provide the way to test the core.
pub trait Frontend {
    type TextObject;
    type Error;
    type TextTextureRegistry: TextTextureRegistry<Error = Self::Error>;
    type AgendaSource: AgendaSource<Error = Self::Error>;

    fn get_hours_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_days_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_dates_text_registry(&mut self) -> &mut Self::TextTextureRegistry;

    fn get_current_week_start(&self) -> Result<calendar::date::Date, Self::Error>;

    fn agenda_source(&self) -> &Self::AgendaSource;
}

/// A trait to fetch the data for the calendar.
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
    // TODO(alex): return a guard which reminds about freeing the handle.
    fn cancel(&self, handle: &Self::RequestHandle);
    fn is_ready(&self, handle: &Self::RequestHandle) -> bool;
    fn free(&self, handle: Self::RequestHandle);
    fn fetch(
        &self,
        handle: &Self::RequestHandle,
        week_start: &calendar::date::Date,
    ) -> calendar::obtain::WeekScheduleWithLanes;
}

// FIXME: Flatten the structure
pub struct WeekData {
    pub agenda: calendar::obtain::WeekScheduleWithLanes,
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

fn register_event_titles<Str, TTC: TextTextureRegistry>(
    text_registry: &mut TTC,
    titles: &[Str],
    rectangles: &[calendar::render::Rectangle],
) -> Result<(), TTC::Error>
where
    Str: AsRef<str>,
{
    assert_eq!(titles.len(), rectangles.len());
    for item in titles.iter().zip(rectangles.iter()) {
        let (title, rectangle): (&Str, &calendar::render::Rectangle) = item;
        let offset_x = config::EVENT_TITLE_OFFSET_X;
        let offset_y = config::EVENT_TITLE_OFFSET_Y;
        let dstrect = FRect {
            x: rectangle.at.x + offset_x,
            y: rectangle.at.y + offset_y,
            w: rectangle.size.x - offset_x * 2f32,
            h: rectangle.size.y - offset_y * 2f32,
        };

        text_registry.create(title.as_ref(), Color::BLACK, dstrect)?;
    }
    Ok(())
}
