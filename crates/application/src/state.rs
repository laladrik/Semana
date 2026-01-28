use std::cell::RefCell;
use std::mem::MaybeUninit;

use crate::{TextRegistry, register_event_titles, render::RenderData};

use super::{DumbObtainAgenda, Error, SdlTextCreate, SurfaceAdjustment, get_current_week_start};
use calendar::ui::{TextObjectFactory, View};

use sdl3_sys::{SDL_FPoint as FPoint, SDL_Point as Point, SDL_Rect as Rect};

pub struct Calendar {
    pub week_start: calendar::date::Date,
    pub week_data: WeekData<sdlext::Text>,
    pub short_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_clash_size: calendar::Lane,
    pub is_week_switched: bool,
}

fn wrap_error(e: WeekDataError<sdlext::TtfError, crate::AgendaObtainError>) -> Error {
    match e {
        WeekDataError::Validate(e) => Error::from(sdlext::Error::from(e)),
        WeekDataError::Obtain(e) => Error::from(e),
    }
}

impl Calendar {
    pub fn new(ui_text_factory: &SdlTextCreate) -> Result<Self, Error> {
        let week_start: calendar::date::Date =
            get_current_week_start().map_err(sdlext::Error::from)?;
        let week_data = WeekData::try_new(&week_start, &DumbObtainAgenda, ui_text_factory)
            .map_err(wrap_error)?;

        let short_event_rectangles_opt: Option<calendar::render::Rectangles> = None;
        let pinned_rectangles_opt: Option<calendar::render::Rectangles> = None;

        // The number of long events making the biggest clash.
        let long_event_clash_size: calendar::Lane = week_data.agenda.long.calculate_biggest_clash();
        let is_week_switched = false;
        Ok(Self {
            week_start,
            week_data,
            short_event_rectangles_opt,
            long_event_rectangles_opt: pinned_rectangles_opt,
            long_event_clash_size,
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

    pub fn update_week_data(&mut self, ui_text_factory: &SdlTextCreate<'_>) -> Result<(), Error> {
        self.week_data = WeekData::try_new(&self.week_start, &DumbObtainAgenda, ui_text_factory)
            .map_err(wrap_error)?;
        self.long_event_clash_size = self.week_data.agenda.long.calculate_biggest_clash();
        self.is_week_switched = false;
        self.drop_events();
        Ok(())
    }

    pub fn drop_events(&mut self) {
        self.long_event_rectangles_opt = None;
        self.short_event_rectangles_opt = None;
    }
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

    pub fn calculate_viewport(&self, window_size: &Point) -> Rect {
        Rect {
            x: self.event_offset.x as i32,
            y: self.event_offset.y as i32,
            w: window_size.x - self.event_offset.x as i32,
            h: window_size.y - self.event_offset.y as i32,
        }
    }
}

// The state of the application
pub struct App {
    pub calendar: Calendar,
    pub ui: UserInterface,
}

impl App {
    pub fn new(
        ui_text_factory: &SdlTextCreate,
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
    ) -> Result<Self, Error> {
        let ui = UserInterface::new(title_font_height, event_offset);
        let calendar = Calendar::new(ui_text_factory)?;
        Ok(Self { calendar, ui })
    }

    pub fn create_view(&mut self, window_size: &Point) -> View {
        View::new(
            FPoint {
                x: window_size.x as f32 - self.ui.event_offset.x,
                y: window_size.y as f32 - self.ui.event_offset.y,
            },
            &mut self.ui.adjustment,
            self.ui.title_font_height,
            self.calendar.long_event_clash_size,
        )
    }

    pub fn create_render_data<'a, 'b>(
        &'a mut self,
        window_size: Point,
        long_event_text_registry: &'b mut TextRegistry,
        short_event_text_registry: &'b mut TextRegistry,
        title_font: &RefCell<sdlext::Font>,
    ) -> Result<RenderData<'a, 'b>, Error> {
        let event_viewport = self.ui.calculate_viewport(&window_size);
        let view = self.create_view(&window_size);
        if self.calendar.long_event_rectangles_opt.is_none() {
            create_long_events(
                &mut self.calendar,
                long_event_text_registry,
                &view,
                title_font,
            )?;
        };

        if self.calendar.short_event_rectangles_opt.is_none() {
            create_short_events(
                &mut self.calendar,
                short_event_text_registry,
                &view,
                title_font,
            )?;
        }

        let short_event_rectangles = self.calendar.short_event_rectangles_opt.as_ref().unwrap();
        let long_event_rectangles = self.calendar.long_event_rectangles_opt.as_ref().unwrap();

        Ok(RenderData {
            view,
            window_size,
            long_event_rectangles,
            week_data: &self.calendar.week_data,
            short_event_rectangles,
            long_event_text_registry,
            short_event_text_registry,
            event_viewport,
            event_offset: self.ui.event_offset,
        })
    }
}

pub fn create_long_events(
    calendar: &mut Calendar,
    text_registry: &mut TextRegistry,
    view: &View,
    title_font: &RefCell<sdlext::Font>,
) -> Result<(), Error> {
    let replacement = calendar::ui::create_long_event_rectangles(
        &view.event_surface,
        &calendar.week_data.agenda.long,
        &calendar.week_start,
        view.cell_width,
        view.calculate_top_panel_height(),
    );

    text_registry.clear();
    register_event_titles(
        text_registry,
        title_font,
        &calendar.week_data.agenda.long.titles,
        &replacement,
    )?;
    calendar.long_event_rectangles_opt = Some(replacement);
    Ok(())
}

pub fn create_short_events(
    calendar: &mut Calendar,
    text_registry: &mut TextRegistry,
    view: &View,
    title_font: &RefCell<sdlext::Font>,
) -> Result<(), Error> {
    let new_rectangles = calendar::ui::create_short_event_rectangles(
        &view.grid_rectangle,
        &calendar.week_data.agenda.short,
        &calendar.week_start,
    );

    text_registry.clear();
    register_event_titles(
        text_registry,
        title_font,
        &calendar.week_data.agenda.short.titles,
        &new_rectangles,
    )?;
    calendar.short_event_rectangles_opt = Some(new_rectangles);
    Ok(())
}

pub struct WeekData<Text> {
    pub agenda: calendar::obtain::WeekScheduleWithLanes,
    pub week: calendar::ui::Week<Text>,
}

enum WeekDataError<V, O> {
    Validate(V),
    Obtain(O),
}

impl<Text> WeekData<Text> {
    const DAYS: u8 = 7;

    fn try_new<TF, AO, ValidateErr, ObtainErr>(
        week_start: &calendar::date::Date,
        agenda_obtain: &AO,
        ui_text_factory: &TF,
    ) -> Result<Self, WeekDataError<ValidateErr, ObtainErr>>
    where
        TF: calendar::TextCreate<Result = Result<Text, ValidateErr>>,
        AO: crate::ObtainAgenda<Error = ObtainErr>,
    {
        let week: calendar::ui::Week<Text> = {
            let stream = calendar::date::DateStream::new(week_start.clone()).take(Self::DAYS as _);
            let week: calendar::ui::Week<TF::Result> =
                calendar::ui::UI::<TF, TF::Result>::create_texts(ui_text_factory, stream);

            match validate_week::<Text, ValidateErr>(week) {
                Ok(x) => x,
                Err(e) => return Err(WeekDataError::Validate(e)),
            }
        };

        match agenda_obtain.obtain_agenda(week_start) {
            Ok(agenda) => Ok(Self { agenda, week }),
            Err(e) => Err(WeekDataError::Obtain(e)),
        }
    }
}

fn validate_array<const N: usize, Text, Err>(
    array: [Result<Text, Err>; N],
) -> Result<[Text; N], Err> {
    unsafe {
        let mut out: MaybeUninit<[Text; N]> = MaybeUninit::uninit();
        let ptr: *mut Text = out.as_mut_ptr() as *mut _;
        for (i, elem) in array.into_iter().enumerate() {
            // SAFETY: the index can't go beyond the array boundaries, because `array` has the same
            // size as `out`.
            ptr.add(i).write(elem?);
        }

        Ok(out.assume_init())
    }
}

fn validate_week<T, E>(
    dirty: calendar::ui::Week<Result<T, E>>,
) -> Result<calendar::ui::Week<T>, E> {
    Ok(calendar::ui::Week {
        days: validate_array(dirty.days)?,
        hours: validate_array(dirty.hours)?,
        dates: validate_array(dirty.dates)?,
    })
}
