use std::{cell::RefCell, mem::MaybeUninit};

use calendar::ui::{SurfaceAdjustment, View};
use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
use sdlext::Ptr;

use sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init};
mod render;

fn get_current_week_start() -> Result<calendar::date::Date, TimeError> {
    sdlext::get_current_time().and_then(date::get_week_start)
}

struct SdlTextCreate<'a> {
    engine: *mut sdl_ttf::TTF_TextEngine,
    font: &'a RefCell<sdlext::Font>,
}

impl calendar::TextCreate for SdlTextCreate<'_> {
    type Result = Result<sdlext::Text, sdlext::TtfError>;

    fn text_create(&self, s: &str) -> Self::Result {
        let cstring = std::ffi::CString::new(s).unwrap();
        sdlext::Text::try_new(self.engine, &mut self.font.borrow_mut(), cstring.as_c_str())
    }
}

struct SdlTextRender;

impl calendar::render::TextRender for SdlTextRender {
    type Text = sdlext::Text;

    type Result = Result<(), sdlext::TtfError>;

    fn text_render(&self, text: &Self::Text, x: f32, y: f32) -> Self::Result {
        unsafe {
            if !sdl_ttf::TTF_DrawRendererText(text.ptr(), x, y) {
                Err(sdlext::TtfError::TextIsNotDrown)
            } else {
                Ok(())
            }
        }
    }
}

mod date;

type MaybeText = Result<sdlext::Text, sdlext::TtfError>;
fn validate_array<const N: usize>(
    array: [MaybeText; N],
) -> Result<[sdlext::Text; N], sdlext::TtfError> {
    unsafe {
        let mut out: MaybeUninit<[sdlext::Text; N]> = MaybeUninit::uninit();
        let ptr: *mut sdlext::Text = out.as_mut_ptr() as *mut _;
        for (i, elem) in array.into_iter().enumerate() {
            // SAFETY: the index can't go beyond the array boundaries, because `array` has the same
            // size as `out`.
            ptr.add(i).write(elem?);
        }

        Ok(out.assume_init())
    }
}

type Week = calendar::ui::Week<sdlext::Text>;

fn validate_week(
    dirty: calendar::ui::Week<Result<sdlext::Text, sdlext::TtfError>>,
) -> Result<Week, sdlext::Error> {
    Ok(Week {
        days: validate_array(dirty.days)?,
        hours: validate_array(dirty.hours)?,
        dates: validate_array(dirty.dates)?,
    })
}

struct TextRegistry<'a> {
    surfaces: Vec<sdlext::Surface>,
    textures: Vec<sdlext::Texture>,
    text_positions: Vec<sdl::SDL_FRect>,
    renderer: &'a sdlext::Renderer,
}

mod config {
    pub const EVENT_TITLE_OFFSET_X: f32 = 2.0;
    pub const EVENT_TITLE_OFFSET_Y: f32 = 4.0;
    pub static FONT_PATH: &std::ffi::CStr = c"assets/DejaVuSansMonoBook.ttf";
    pub const COLOR_BACKGROUND: u32 = 0x0C0D0C;
    pub const COLOR_EVENT_TITLE: u32 = 0x000000;
    pub const GRID_SCALE_STEP: f32 = 50.;
    pub const GRID_OFFSET_STEP: f32 = 50.;
}

impl<'a> TextRegistry<'a> {
    fn new(renderer: &'a sdlext::Renderer) -> Self {
        Self {
            renderer,
            surfaces: Vec::new(),
            textures: Vec::new(),
            text_positions: Vec::new(),
        }
    }

    fn create(
        &mut self,
        text: &std::ffi::CStr,
        font: &RefCell<Font>,
        position: sdl::SDL_FRect,
    ) -> Result<(), sdlext::Error> {
        unsafe {
            let wrap_length: i32 = {
                let p = position.w.floor();
                assert!(p <= i32::MAX as f32);
                p as i32
            };

            let surf: sdlext::Surface = sdlext::ttf_render_text_blended_wrapped(
                &mut font.borrow_mut(),
                text,
                Color::from_rgb(config::COLOR_EVENT_TITLE).into(),
                wrap_length,
            )?;

            let texture: sdlext::Texture =
                sdlext::create_texture_from_surface(self.renderer, &surf)?;

            let pos = {
                let (texture_width, texture_height): (f32, f32) = {
                    let mut width = 0f32;
                    let mut height = 0f32;
                    if !sdl::SDL_GetTextureSize(texture.ptr(), &mut width, &mut height) {
                        panic!("the texture size failed to be obtained");
                    }
                    (width, height)
                };

                sdl::SDL_FRect {
                    x: position.x,
                    y: position.y,
                    w: texture_width.min(position.w as _),
                    h: texture_height.min(position.h as _),
                }
            };

            self.surfaces.push(surf);
            self.textures.push(texture);
            self.text_positions.push(pos);
        }
        Ok(())
    }

    fn render(&self) -> Result<(), sdlext::Error> {
        for (texture, position) in self.textures.iter().zip(self.text_positions.iter()) {
            let src = sdl::SDL_FRect {
                x: 0f32,
                y: 0f32,
                w: position.w,
                h: position.h,
            };

            self.renderer.render_texture(texture, &src, position)?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.surfaces.clear();
        self.textures.clear();
        self.text_positions.clear();
    }
}

impl<'a> Drop for TextRegistry<'a> {
    fn drop(&mut self) {
        self.clear()
    }
}

struct Fonts {
    title: RefCell<Font>,
    ui: RefCell<Font>,
}

impl Fonts {
    fn new(
        title_font_path: &std::ffi::CStr,
        ui_font_path: &std::ffi::CStr,
    ) -> Result<Self, sdlext::Error> {
        let title_font: RefCell<Font> = Font::open(title_font_path, 16.0).map(RefCell::new)?;

        let ui_font: RefCell<Font> = Font::open(ui_font_path, 22.0).map(RefCell::new)?;
        Ok(Self {
            title: title_font,
            ui: ui_font,
        })
    }
}

#[derive(Debug)]
struct CalendarError {
    _data: String,
}

impl<'event> From<calendar::Error<'event>> for CalendarError {
    fn from(value: calendar::Error<'event>) -> Self {
        let (calendar::Error::InvalidDate(data) | calendar::Error::InvalidTime(data)) = value;
        Self {
            _data: data.to_owned(),
        }
    }
}

type JsonParseError = <calendar::obtain::NanoSerde as calendar::obtain::JsonParser>::Error;
type AgendaObtainError = calendar::obtain::Error<JsonParseError>;

#[derive(Debug)]
#[allow(unused)]
enum Error {
    Sdl(sdlext::Error),
    Calendar(CalendarError),
    DataIsNotAvailable(AgendaObtainError),
}

impl From<sdlext::Error> for Error {
    fn from(value: sdlext::Error) -> Self {
        Error::Sdl(value)
    }
}

impl From<CalendarError> for Error {
    fn from(value: CalendarError) -> Self {
        Error::Calendar(value)
    }
}

fn register_event_titles<Str>(
    text_registry: &mut TextRegistry,
    font: &RefCell<Font>,
    titles: &[Str],
    rectangles: &[calendar::render::Rectangle],
) -> Result<(), Error>
where
    Str: AsRef<str>,
{
    assert_eq!(titles.len(), rectangles.len());
    for item in titles.iter().zip(rectangles.iter()) {
        let (title, rectangle): (&Str, &calendar::render::Rectangle) = item;
        let c_title =
            std::ffi::CString::new(title.as_ref()).expect("can't create c string for an event");
        let offset_x = config::EVENT_TITLE_OFFSET_X;
        let offset_y = config::EVENT_TITLE_OFFSET_Y;
        let dstrect = sdl::SDL_FRect {
            x: rectangle.at.x + offset_x,
            y: rectangle.at.y + offset_y,
            w: rectangle.size.x - offset_x * 2f32,
            h: rectangle.size.y - offset_y * 2f32,
        };

        text_registry.create(c_title.as_c_str(), font, dstrect)?;
    }
    Ok(())
}

fn obtain_agenda(
    week_start: &calendar::date::Date,
) -> Result<calendar::obtain::WeekScheduleWithLanes, AgendaObtainError> {
    let mut arguments = calendar::obtain::khal::week_arguments(week_start);
    let bin: Result<String, _> = std::env::var("SEMANA_BACKEND_BIN");
    if let Ok(ref v) = bin {
        arguments.backend_bin_path = v.as_ref();
    }

    calendar::obtain::events_with_lanes(
        &calendar::obtain::EventSourceStd,
        &calendar::obtain::NanoSerde,
        &arguments,
    )
}

struct WeekData {
    agenda: calendar::obtain::WeekScheduleWithLanes,
    week: Week,
}

impl WeekData {
    const DAYS: u8 = 7;

    fn try_new(
        week_start: &calendar::date::Date,
        ui_text_factory: &SdlTextCreate,
    ) -> Result<Self, Error> {
        let week: Week = {
            let stream = calendar::date::DateStream::new(week_start.clone()).take(Self::DAYS as _);
            let week: calendar::ui::Week<Result<sdlext::Text, _>> =
                calendar::ui::create_texts(ui_text_factory, stream);
            validate_week(week)?
        };

        let agenda: calendar::obtain::WeekScheduleWithLanes =
            obtain_agenda(week_start).map_err(Error::DataIsNotAvailable)?;
        Ok(Self { agenda, week })
    }
}

mod state {
    use super::{Error, SdlTextCreate, SurfaceAdjustment, WeekData, get_current_week_start};
    pub struct Calendar {
        pub week_start: calendar::date::Date,
        pub week_data: WeekData,
        pub short_event_rectangles_opt: Option<calendar::render::Rectangles>,
        pub pinned_rectangles_opt: Option<calendar::render::Rectangles>,
        pub long_event_clash_size: calendar::Lane,
        pub is_week_switched: bool,
    }

    impl Calendar {
        pub fn new(ui_text_factory: &SdlTextCreate) -> Result<Self, Error> {
            let week_start: calendar::date::Date =
                get_current_week_start().map_err(sdlext::Error::from)?;
            let week_data = WeekData::try_new(&week_start, ui_text_factory)?;

            let short_event_rectangles_opt: Option<calendar::render::Rectangles> = None;
            let pinned_rectangles_opt: Option<calendar::render::Rectangles> = None;

            // The number of long events making the biggest clash.
            let long_event_clash_size: calendar::Lane =
                week_data.agenda.long.calculate_biggest_clash();
            let is_week_switched = false;
            Ok(Self {
                week_start,
                week_data,
                short_event_rectangles_opt,
                pinned_rectangles_opt,
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

        pub fn update_week_data(
            &mut self,
            ui_text_factory: &SdlTextCreate<'_>,
        ) -> Result<(), Error> {
            self.week_data = WeekData::try_new(&self.week_start, ui_text_factory)?;
            self.long_event_clash_size = self.week_data.agenda.long.calculate_biggest_clash();
            self.is_week_switched = false;
            self.pinned_rectangles_opt.take();
            self.short_event_rectangles_opt.take();
            Ok(())
        }
    }

    pub struct UserInterface {
        pub adjustment: SurfaceAdjustment,
        pub title_font_height: std::ffi::c_int,
    }

    impl UserInterface {
        fn new(title_font_height: std::ffi::c_int) -> Result<Self, Error> {
            // the values to scale and scroll the events grid (short events).
            let adjustment = SurfaceAdjustment {
                vertical_scale: 0.,
                vertical_offset: 0.,
            };

            Ok(Self {
                adjustment,
                title_font_height,
            })
        }

        pub fn add_adjustment(&mut self, value: f32) {
            self.adjustment.vertical_offset -= value;
            self.adjustment.vertical_offset = self
                .adjustment
                .vertical_offset
                .clamp(-self.adjustment.vertical_scale, 0f32);
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
        ) -> Result<Self, Error> {
            let ui = UserInterface::new(title_font_height)?;
            let calendar = Calendar::new(ui_text_factory)?;
            Ok(Self { calendar, ui })
        }
    }
}

use state::App;

fn unsafe_main() {
    unsafe {
        let ret: Result<(), Error> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: &sdlext::Renderer| {
                let mut short_event_text_registry = TextRegistry::new(renderer);
                let mut long_event_text_registry = TextRegistry::new(renderer);
                let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
                _ = sdl::SDL_GetWindowSize(root_window, &mut window_size.x, &mut window_size.y);

                sdl_ttf_init(
                    renderer,
                    move |engine: *mut sdl_ttf::TTF_TextEngine| -> Result<(), Error> {
                        let fonts = Fonts::new(config::FONT_PATH, config::FONT_PATH)?;
                        let title_font_height: std::ffi::c_int =
                            sdl_ttf::TTF_GetFontHeight(fonts.title.borrow_mut().ptr());
                        let ui_text_factory = SdlTextCreate {
                            engine,
                            font: &fonts.ui,
                        };
                        let mut app = App::new(&ui_text_factory, title_font_height)?;

                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        'outer_loop: loop {
                            // stage: event handle
                            while sdl::SDL_PollEvent(&mut event as _) {
                                match event.type_ {
                                    sdl::SDL_EVENT_QUIT => break 'outer_loop,
                                    sdl::SDL_EVENT_WINDOW_RESIZED => {
                                        app.calendar.pinned_rectangles_opt.take();
                                        app.calendar.short_event_rectangles_opt.take();
                                        _ = sdl::SDL_GetWindowSize(
                                            root_window,
                                            &mut window_size.x,
                                            &mut window_size.y,
                                        );
                                    }
                                    sdl::SDL_EVENT_KEY_DOWN => match event.key.key {
                                        sdl::SDLK_UP => {
                                            app.ui.add_adjustment(-config::GRID_OFFSET_STEP);
                                            app.calendar.pinned_rectangles_opt.take();
                                            app.calendar.short_event_rectangles_opt.take();
                                        }
                                        sdl::SDLK_DOWN => {
                                            app.ui.add_adjustment(config::GRID_OFFSET_STEP);
                                            app.calendar.pinned_rectangles_opt.take();
                                            app.calendar.short_event_rectangles_opt.take();
                                        }
                                        sdl::SDLK_MINUS => {
                                            app.ui.adjustment.vertical_scale = 0f32.max(
                                                app.ui.adjustment.vertical_scale
                                                    - config::GRID_SCALE_STEP,
                                            );
                                            app.calendar.pinned_rectangles_opt.take();
                                            app.calendar.short_event_rectangles_opt.take();
                                        }
                                        sdl::SDLK_EQUALS => {
                                            app.ui.adjustment.vertical_scale +=
                                                config::GRID_SCALE_STEP;
                                            app.calendar.pinned_rectangles_opt.take();
                                            app.calendar.short_event_rectangles_opt.take();
                                        }
                                        _ => (),
                                    },
                                    sdl::SDL_EVENT_KEY_UP => match event.key.key {
                                        sdl::SDLK_PAGEUP => app.calendar.subtract_week(),
                                        sdl::SDLK_PAGEDOWN => app.calendar.add_week(),
                                        _ => (),
                                    },
                                    _ => (),
                                }
                            }

                            if app.calendar.is_week_switched {
                                app.calendar.update_week_data(&ui_text_factory)?;
                            }

                            assert!(
                                app.calendar.long_event_clash_size as usize
                                    <= app.calendar.week_data.agenda.long.event_ranges.len(),
                                "the size of long events' clash can't be bigger than the number of the long events",
                            );

                            let event_offset = sdl::SDL_FPoint {
                                x: 100f32,
                                y: 70f32,
                            };

                            //let event_surface = view.event_surface;
                            let event_viewport = sdl::SDL_Rect {
                                x: event_offset.x as i32,
                                y: event_offset.y as i32,
                                w: window_size.x - event_offset.x as i32,
                                h: window_size.y - event_offset.y as i32,
                            };

                            let view = View::new(
                                sdl::SDL_FPoint {
                                    x: window_size.x as f32 - event_offset.x,
                                    y: window_size.y as f32 - event_offset.y,
                                },
                                &mut app.ui.adjustment,
                                app.ui.title_font_height,
                                app.calendar.long_event_clash_size,
                            );

                            let long_event_rectangles: &calendar::render::Rectangles = {
                                let ret: Result<&calendar::render::Rectangles, CalendarError> =
                                    match app.calendar.pinned_rectangles_opt {
                                        Some(ref x) => Ok(x),
                                        None => {
                                            let replacement =
                                                calendar::ui::create_long_event_rectangles(
                                                    &view.event_surface,
                                                    &app.calendar.week_data.agenda.long,
                                                    &app.calendar.week_start,
                                                    view.cell_width,
                                                    view.calculate_top_panel_height(),
                                                );
                                            // TODO: implement a facility which creates the titles
                                            // of the events at once for the "All day" events and
                                            // regular events.  This would allow to prevent
                                            // accidential calling of `clear` twice.
                                            long_event_text_registry.clear();
                                            register_event_titles(
                                                &mut long_event_text_registry,
                                                &fonts.title,
                                                &app.calendar.week_data.agenda.long.titles,
                                                &replacement,
                                            )?;
                                            Ok(app
                                                .calendar
                                                .pinned_rectangles_opt
                                                .get_or_insert(replacement))
                                        }
                                    };

                                ret?
                            };

                            if app.calendar.short_event_rectangles_opt.is_none() {
                                let new_rectangles = calendar::ui::create_short_event_rectangles(
                                    &view.grid_rectangle,
                                    &app.calendar.week_data.agenda.short,
                                    &app.calendar.week_start,
                                );
                                short_event_text_registry.clear();
                                register_event_titles(
                                    &mut short_event_text_registry,
                                    &fonts.title,
                                    &app.calendar.week_data.agenda.short.titles,
                                    &new_rectangles,
                                )?;

                                app.calendar
                                    .short_event_rectangles_opt
                                    .replace(new_rectangles);
                            }

                            let short_event_rectangles =
                                app.calendar.short_event_rectangles_opt.as_ref().unwrap();

                            let data = &render::RenderData {
                                view,
                                window_size,
                                long_event_rectangles,
                                week_data: &app.calendar.week_data,
                                short_event_rectangles,
                                long_event_text_registry: &long_event_text_registry,
                                short_event_text_registry: &short_event_text_registry,
                                event_viewport,
                                event_offset,
                            };

                            /* stage: render */
                            render::render(renderer, data)?;
                        }

                        let _ = root_window;
                        Ok(())
                    },
                )
            },
        );

        if let Err(err) = ret {
            println!("The application failed with the error {:?}", err);
        }
    }
}

struct RectangleRender<'a> {
    renderer: &'a sdlext::Renderer,
}

impl<'a> calendar::render::RenderRectangles for RectangleRender<'a> {
    type Result = Result<(), sdlext::Error>;

    fn render_rectangles<'r, I>(&self, rectangles: I) -> Self::Result
    where
        I: Iterator<Item = &'r calendar::render::Rectangle>,
    {
        for rect in rectangles {
            self.renderer
                .set_render_draw_color(calendar_color_2_to_sdl_color(rect.color))?;
            let sdl_rect = create_sdl_frect(rect);
            self.renderer.render_fill_rect(&sdl_rect)?;

            let border = sdl::SDL_FRect {
                x: sdl_rect.x,
                y: sdl_rect.y,
                w: sdl_rect.w,
                h: 5.0,
            };

            self.renderer
                .set_render_draw_color(Color::from_rgb(0xff0000))?;
            self.renderer.render_fill_rect(&border)?;
        }
        Ok(())
    }
}

fn create_sdl_frect(from: &calendar::render::Rectangle) -> sdl::SDL_FRect {
    sdl::SDL_FRect {
        x: from.at.x,
        y: from.at.y,
        w: from.size.x,
        h: from.size.y,
    }
}

fn main() {
    unsafe_main();
}

fn calendar_color_2_to_sdl_color(value: calendar::Color) -> sdlext::Color {
    let value = u32::from(value);
    sdlext::Color {
        r: (value >> 24) as u8,
        g: (value >> 16) as u8,
        b: (value >> 8) as u8,
        a: 0xff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_start_week() {
        // 2025-10-10 is Friday
        let now_date = sdl::SDL_DateTime {
            year: 2025,
            month: 10,
            day: 10,
            hour: 0,
            minute: 40,
            second: 00,
            nanosecond: 00,
            day_of_week: 5,
            utc_offset: 1,
        };

        unsafe {
            let mut now_time: sdl::SDL_Time = std::mem::zeroed();
            assert!(sdl::SDL_DateTimeToTime(&now_date, &mut now_time));
            let res = date::get_week_start(now_time)
                .expect("getting the start of the week must not fail");
            let calendar::date::Date { year, month, day } = res;
            assert_eq!(2025, year);
            assert_eq!(10, month);
            assert_eq!(6, day);
        }
    }
}
