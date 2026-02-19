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

                let long_event_rectangles_opt = create_long_events(
                    &week_data.agenda.long,
                    &self.week_start,
                    long_event_text_registry,
                    view,
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
}

static NO_RECT: calendar::render::Rectangles = Vec::new();
struct EventRectangles<'rect> {
    long: &'rect calendar::render::Rectangles,
    short: &'rect calendar::render::Rectangles,
}

pub struct UserInterface {
    pub adjustment: SurfaceAdjustment,
    pub title_font_height: std::ffi::c_int,
    pub event_offset: FPoint,
}

impl UserInterface {
    fn new(title_font_height: std::ffi::c_int, event_offset: FPoint) -> Self {
        // the values to scale and scroll the events grid (short events).
        let adjustment = SurfaceAdjustment {
            vertical_scale: 0.,
            vertical_offset: 0.,
        };

        Self {
            adjustment,
            title_font_height,
            event_offset,
        }
    }

    pub fn add_adjustment(&mut self, value: f32) {
        self.adjustment.vertical_offset -= value;
        self.adjustment.vertical_offset = self
            .adjustment
            .vertical_offset
            .clamp(-self.adjustment.vertical_scale, 0f32);
    }

    fn calculate_viewport(&self, window_size: &Point) -> Rect {
        Rect {
            x: self.event_offset.x as i32,
            y: self.event_offset.y as i32,
            w: window_size.x - self.event_offset.x as i32,
            h: window_size.y - self.event_offset.y as i32,
        }
    }
}

// The state of the application
pub struct App<F: Frontend> {
    pub calendar: Calendar<F>,
    pub ui: UserInterface,
}

const DUMB_CELL_WIDTH: f32 = 130f32;

impl<F: Frontend> App<F> {
    pub fn new(
        frontend: &mut F,
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
    ) -> Result<Self, F::Error> {
        let ui = UserInterface::new(title_font_height, event_offset);
        let calendar = Calendar::new(frontend)?;
        App::create_hours_text_objects(frontend, ui.event_offset.x)?;

        let cell_width = DUMB_CELL_WIDTH; // FIXME: the value must be calculated
        App::create_days_text_objects(frontend, cell_width)?;

        App::create_dates_text_objects(frontend, cell_width, &calendar.week_start)?;
        Ok(Self { calendar, ui })
    }

    fn create_view(&mut self, window_size: &Point) -> View {
        let clash_size: u8 = match self.calendar.state {
            CalendarState::Loading { .. } => 0,
            CalendarState::Ready {
                long_event_clash_size,
                ..
            }
            | CalendarState::Rendering {
                long_event_clash_size,
                ..
            } => long_event_clash_size,
        };

        View::new(
            FPoint {
                x: window_size.x as f32 - self.ui.event_offset.x,
                y: window_size.y as f32 - self.ui.event_offset.y,
            },
            &mut self.ui.adjustment,
            self.ui.title_font_height,
            clash_size,
        )
    }

    fn reposition_hours_text_objects(&self, frontend: &mut F, width: f32, view: &View) {
        let cell_height = view.cell_height;
        let offset_y = view.grid_rectangle.y - view.calculate_top_panel_height();
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
    ) -> Result<RenderData<'wdrect, 'ttc, F::TextTextureRegistry, F>, F::Error> {
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

        let event_viewport: Rect = self.ui.calculate_viewport(&window_size);
        let view = self.create_view(&window_size);
        let hours_viewport: Rect = {
            let y = event_viewport.y + view.calculate_top_panel_height() as i32;
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
        self.calendar
            .get_ready(&view, long_event_text_registry, short_event_text_registry)?;
        let rectangles: EventRectangles = self.calendar.state.obtain_events();
        Ok(RenderData {
            view,
            long_event_rectangles: rectangles.long,
            hours_viewport,
            dates_viewport,
            short_event_rectangles: rectangles.short,
            long_event_text_registry,
            short_event_text_registry,
            event_viewport,
            frontend,
        })
    }
}

pub fn create_long_events<TTC: TextTextureRegistry>(
    event_data: &calendar::EventData,
    week_start: &calendar::date::Date,
    text_registry: &mut TTC,
    view: &View,
) -> Result<calendar::render::Rectangles, TTC::Error> {
    let replacement = calendar::ui::create_long_event_rectangles(
        &view.event_surface,
        event_data,
        week_start,
        view.cell_width,
        view.calculate_top_panel_height(),
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
    let new_rectangles =
        calendar::ui::create_short_event_rectangles(&view.grid_rectangle, event_data, week_start);

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
