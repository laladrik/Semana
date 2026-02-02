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
    pub week_data: WeekData,
    pub short_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_rectangles_opt: Option<calendar::render::Rectangles>,
    pub long_event_clash_size: calendar::Lane,
    pub is_week_switched: bool,
}

impl<F: Frontend> Calendar<F> {
    fn new(frontend: &F) -> Result<Self, F::Error> {
        let week_start: calendar::date::Date = frontend.get_current_week_start()?;
        let week_data = WeekData::try_new(&week_start, frontend)?;

        let short_event_rectangles_opt: Option<calendar::render::Rectangles> = None;
        let pinned_rectangles_opt: Option<calendar::render::Rectangles> = None;

        // The number of long events making the biggest clash.
        let long_event_clash_size: calendar::Lane = week_data.agenda.long.calculate_biggest_clash();
        let is_week_switched = false;
        Ok(Self {
            _frontend: std::marker::PhantomData,
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

    fn update_week_data(&mut self, frontend: &F) -> Result<(), F::Error> {
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

impl<F: Frontend> App<F> {
    pub fn new(
        frontend: &mut F,
        title_font_height: std::ffi::c_int,
        title_font: &<F::TextTextureRegistry as TextTextureRegistry>::Font,
        event_offset: FPoint,
    ) -> Result<Self, F::Error> {
        let ui = UserInterface::new(title_font_height, event_offset);
        let calendar = Calendar::new(frontend)?;
        App::create_hours_text_objects(frontend, ui.event_offset.x, title_font)?;

        let cell_width = 100f32; // FIXME: the value must be calculated
        App::create_days_text_objects(frontend, cell_width, title_font)?;

        App::create_dates_text_objects(frontend, cell_width, title_font, &calendar.week_start)?;
        Ok(Self { calendar, ui })
    }

    fn create_view(&mut self, window_size: &Point) -> View {
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

    fn create_hours_text_objects(
        frontend: &mut F,
        panel_width: f32,
        title_font: &<F::TextTextureRegistry as TextTextureRegistry>::Font,
    ) -> Result<(), F::Error> {
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

            hours_registry.create(s.as_str(), title_font, Color::WHITE, position)?;
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

    fn create_days_text_objects(
        frontend: &mut F,
        cell_width: f32,
        title_font: &<<F as Frontend>::TextTextureRegistry as TextTextureRegistry>::Font,
    ) -> Result<(), F::Error> {
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
            days_registry.create(day, title_font, Color::WHITE, position)?;
        }
        Ok(())
    }

    fn create_dates_text_objects(
        frontend: &mut F,
        cell_width: f32,
        title_font: &<<F as Frontend>::TextTextureRegistry as TextTextureRegistry>::Font,
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
            dates_registry.create(text, title_font, Color::WHITE, position)?;
        }

        Ok(())
    }

    pub fn create_render_data<'wdrect, 'ttc>(
        &'wdrect mut self,
        frontend: &'ttc mut F,
        window_size: Point,
        long_event_text_registry: &'ttc mut F::TextTextureRegistry,
        short_event_text_registry: &'ttc mut F::TextTextureRegistry,
        title_font: &<F::TextTextureRegistry as TextTextureRegistry>::Font,
    ) -> Result<RenderData<'wdrect, 'ttc, F::TextTextureRegistry, F>, F::Error> {
        if self.calendar.is_week_switched {
            self.calendar.update_week_data(frontend)?;
            // FIXME the cell width should not affect
            let cell_width = 100f32;
            frontend.get_dates_text_registry().clear();
            App::create_dates_text_objects(
                frontend,
                cell_width,
                title_font,
                &self.calendar.week_start,
            )?;
        }

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
        assert!(
            self.calendar.long_event_clash_size as usize
                <= self.calendar.week_data.agenda.long.event_ranges.len(),
            "the size of long events' clash can't be bigger than the number of the long events",
        );

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
            long_event_rectangles,
            hours_viewport,
            dates_viewport,
            short_event_rectangles,
            long_event_text_registry,
            short_event_text_registry,
            event_viewport,
            frontend,
        })
    }
}

pub fn create_long_events<F: Frontend, TTC: TextTextureRegistry>(
    calendar: &mut Calendar<F>,
    text_registry: &mut TTC,
    view: &View,
    title_font: &TTC::Font,
) -> Result<(), TTC::Error> {
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

fn create_short_events<F: Frontend, TTC: TextTextureRegistry>(
    calendar: &mut Calendar<F>,
    text_registry: &mut TTC,
    view: &View,
    title_font: &TTC::Font,
) -> Result<(), TTC::Error> {
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

/// The trait provides the platform dependant functionality.  The main purpose of the abstraction
/// is provide the way to test the core.
pub trait Frontend: calendar::TextCreate<Result = Result<Self::TextObject, Self::Error>> {
    type TextObject;
    type Error;
    type TextTextureRegistry: TextTextureRegistry<Error = Self::Error>;

    fn get_hours_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_days_text_registry(&mut self) -> &mut Self::TextTextureRegistry;
    fn get_dates_text_registry(&mut self) -> &mut Self::TextTextureRegistry;

    fn get_current_week_start(&self) -> Result<calendar::date::Date, Self::Error>;

    fn obtain_agenda(
        &self,
        week_start: &calendar::date::Date,
    ) -> Result<calendar::obtain::WeekScheduleWithLanes, Self::Error>;
}

// FIXME: Flatten the structure
pub struct WeekData {
    pub agenda: calendar::obtain::WeekScheduleWithLanes,
}

impl WeekData {
    fn try_new<F, T, E>(week_start: &calendar::date::Date, frontend: &F) -> Result<Self, E>
    where
        F: Frontend<TextObject = T, Error = E>,
    {
        frontend
            .obtain_agenda(week_start)
            .map(|agenda| Self { agenda })
    }
}

/// Stores textures of the text objects.
pub trait TextTextureRegistry {
    type Error;
    type Font;

    /// The method updates the destination rectangles of the textures of the text objects which
    /// were created by [`Self::create`].  The iterator returns as much items as the number of the
    /// created text objects.
    fn update_positions(&mut self, values: impl Iterator<Item = FRect>);

    fn clear(&mut self);

    /// Creates a text object from `text`.  The text object is stored within the registry.
    fn create(
        &mut self,
        text: impl Into<Vec<u8>>,
        font: &Self::Font,
        color: Color,
        position: FRect,
    ) -> Result<(), Self::Error>;
}

fn register_event_titles<Str, TTC: TextTextureRegistry>(
    text_registry: &mut TTC,
    font: &TTC::Font,
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

        text_registry.create(title.as_ref(), font, Color::BLACK, dstrect)?;
    }
    Ok(())
}
