use std::cell::RefCell;
use std::mem::MaybeUninit;

use crate::{Error, TextRegistry, register_event_titles, render::RenderData};

use calendar::ui::{SurfaceAdjustment, TextObjectFactory, View};

use sdl3_sys::{SDL_FPoint as FPoint, SDL_Point as Point, SDL_Rect as Rect};

pub struct Calendar<F: Frontend> {
    pub week_start: calendar::date::Date,
    pub week_data: WeekData<F::TextObject>,
    pub short_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_clash_size: calendar::Lane,
    pub is_week_switched: bool,
}

impl<F: Frontend> Calendar<F> {
    pub fn new(frontend: &F) -> Result<Self, F::Error> {
        let week_start: calendar::date::Date = frontend.get_current_week_start()?;
        let week_data = WeekData::try_new(&week_start, frontend)?;

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

    pub fn update_week_data(&mut self, frontend: &F) -> Result<(), F::Error> {
        self.week_data = WeekData::try_new(&self.week_start, frontend)?;
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
pub struct App<F: Frontend> {
    pub calendar: Calendar<F>,
    pub ui: UserInterface,
}

impl<F: Frontend> App<F> {
    pub fn new(
        frontend: &F,
        title_font_height: std::ffi::c_int,
        event_offset: FPoint,
    ) -> Result<Self, F::Error> {
        let ui = UserInterface::new(title_font_height, event_offset);
        let calendar = Calendar::new(frontend)?;
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
    ) -> Result<RenderData<'a, 'b, F::TextObject>, Error> {
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

pub fn create_long_events<F: Frontend>(
    calendar: &mut Calendar<F>,
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

pub fn create_short_events<F: Frontend>(
    calendar: &mut Calendar<F>,
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

pub trait Frontend: calendar::TextCreate<Result = Result<Self::TextObject, Self::Error>> {
    type TextObject;
    type Error;

    fn get_current_week_start(&self) -> Result<calendar::date::Date, Self::Error>;

    fn obtain_agenda(
        &self,
        week_start: &calendar::date::Date,
    ) -> Result<calendar::obtain::WeekScheduleWithLanes, Self::Error>;
}

pub struct WeekData<TextObject> {
    pub agenda: calendar::obtain::WeekScheduleWithLanes,
    pub week: calendar::ui::Week<TextObject>,
}

impl<Text> WeekData<Text> {
    const DAYS: u8 = 7;

    fn try_new<F, E>(week_start: &calendar::date::Date, frontend: &F) -> Result<Self, E>
    where
        F: Frontend<TextObject = Text, Error = E>,
    {
        let week: calendar::ui::Week<Text> = {
            let stream = calendar::date::DateStream::new(week_start.clone()).take(Self::DAYS as _);
            let week: calendar::ui::Week<Result<Text, E>> =
                calendar::ui::UI::<F, Result<Text, E>>::create_texts(frontend, stream);

            validate_week(week)?
        };

        frontend
            .obtain_agenda(week_start)
            .map(|agenda| Self { agenda, week })
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
