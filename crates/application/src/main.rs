use std::cell::RefCell;

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
use sdlext::Ptr;

use sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init};

mod date;
mod render;

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
/// The registry with the text objects to be rendered.
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

impl From<FrontendError> for Error {
    fn from(value: FrontendError) -> Self {
        match value {
            FrontendError::TextObjectIsNotCreated(e) => Error::from(sdlext::Error::from(e)),
            FrontendError::AgendaIsNotObtained(e) => Error::from(e),
            FrontendError::WeekStartIsNotObtained(e) => Error::from(sdlext::Error::from(e)),
        }
    }
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

impl From<AgendaObtainError> for Error {
    fn from(value: AgendaObtainError) -> Self {
        Error::DataIsNotAvailable(value)
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

struct DumbFrontend<'a, 'b>(&'b SdlTextCreate<'a>);
impl<'a, 'b> calendar::TextCreate for DumbFrontend<'a, 'b> {
    type Result = Result<<Self as Frontend>::TextObject, FrontendError>;

    fn text_create(&self, s: &str) -> Self::Result {
        self.0
            .text_create(s)
            .map_err(FrontendError::TextObjectIsNotCreated)
    }
}

enum FrontendError {
    TextObjectIsNotCreated(sdlext::TtfError),
    AgendaIsNotObtained(AgendaObtainError),
    WeekStartIsNotObtained(TimeError),
}

impl<'a, 'b> Frontend for DumbFrontend<'a, 'b> {
    type TextObject = sdlext::Text;
    type Error = FrontendError;

    fn get_current_week_start(&self) -> Result<calendar::date::Date, FrontendError> {
        sdlext::get_current_time()
            .and_then(date::get_week_start)
            .map_err(FrontendError::WeekStartIsNotObtained)
    }

    fn obtain_agenda(
        &self,
        week_start: &calendar::date::Date,
    ) -> Result<calendar::obtain::WeekScheduleWithLanes, Self::Error> {
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
        .map_err(FrontendError::AgendaIsNotObtained)
    }
}

mod state;

use state::App;

use crate::state::Frontend;

fn unsafe_main() {
    let event_offset = sdl::SDL_FPoint {
        x: 100f32,
        y: 70f32,
    };

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
                        let frontend = DumbFrontend(&ui_text_factory);
                        let mut app = App::new(&frontend, title_font_height, event_offset)?;

                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        'outer_loop: loop {
                            // stage: event handle
                            while sdl::SDL_PollEvent(&mut event as _) {
                                match event.type_ {
                                    sdl::SDL_EVENT_QUIT => break 'outer_loop,
                                    sdl::SDL_EVENT_WINDOW_RESIZED => {
                                        app.calendar.drop_events();
                                        _ = sdl::SDL_GetWindowSize(
                                            root_window,
                                            &mut window_size.x,
                                            &mut window_size.y,
                                        );
                                    }
                                    sdl::SDL_EVENT_KEY_DOWN => match event.key.key {
                                        sdl::SDLK_UP => {
                                            app.ui.add_adjustment(-config::GRID_OFFSET_STEP);
                                            app.calendar.drop_events();
                                        }
                                        sdl::SDLK_DOWN => {
                                            app.ui.add_adjustment(config::GRID_OFFSET_STEP);
                                            app.calendar.drop_events();
                                        }
                                        sdl::SDLK_MINUS => {
                                            app.ui.adjustment.vertical_scale = 0f32.max(
                                                app.ui.adjustment.vertical_scale
                                                    - config::GRID_SCALE_STEP,
                                            );
                                            app.calendar.drop_events();
                                        }
                                        sdl::SDLK_EQUALS => {
                                            app.ui.adjustment.vertical_scale +=
                                                config::GRID_SCALE_STEP;
                                            app.calendar.drop_events();
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
                                app.calendar.update_week_data(&frontend)?;
                            }

                            assert!(
                                app.calendar.long_event_clash_size as usize
                                    <= app.calendar.week_data.agenda.long.event_ranges.len(),
                                "the size of long events' clash can't be bigger than the number of the long events",
                            );

                            let data = app.create_render_data(
                                window_size,
                                &mut long_event_text_registry,
                                &mut short_event_text_registry,
                                &fonts.title,
                            )?;
                            /* stage: render */
                            render::render(renderer, &data)?;
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
