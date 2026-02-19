mod date;
mod error;
mod render;
mod state;

use std::cell::RefCell;

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;

use sdlext::Ptr;
use sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init};

use crate::error::{Error, FrontendError};
use crate::state::AgendaSource;
use state::{App, Frontend};

/// The registry with the textures of the text objects.
struct TextTextureRegistry<'renderer, 'font> {
    font: &'font RefCell<sdlext::Font>,
    surfaces: Vec<sdlext::Surface>,
    textures: Vec<sdlext::Texture>,
    // An element of the vector is the original size of the texture.  It is used to prevent
    // stretching of a texture when a new size comes through
    // [`TextTextureCreate::update_positions`].
    texture_sizes: Vec<sdl::SDL_FPoint>,
    // An element of the vector is the destination rectangle where a texture of a text object is
    // rendered.  Which means that if the size of the destination rectangle is smaller than the
    // size of the respective texture, the texture is cropped.
    text_positions: Vec<sdl::SDL_FRect>,
    renderer: &'renderer sdlext::Renderer,
}

mod config {
    pub const EVENT_TITLE_OFFSET_X: f32 = 2.0;
    pub const EVENT_TITLE_OFFSET_Y: f32 = 4.0;
    pub static FONT_PATH: &std::ffi::CStr = c"assets/DejaVuSansMonoBook.ttf";
    pub const COLOR_BACKGROUND: u32 = 0x0C0D0C;
    pub const GRID_SCALE_STEP: f32 = 50.;
    pub const GRID_OFFSET_STEP: f32 = 50.;
}

impl<'renderer, 'font> TextTextureRegistry<'renderer, 'font> {
    fn new(renderer: &'renderer sdlext::Renderer, font: &'font RefCell<sdlext::Font>) -> Self {
        Self {
            font,
            renderer,
            texture_sizes: Vec::new(),
            surfaces: Vec::new(),
            textures: Vec::new(),
            text_positions: Vec::new(),
        }
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
        self.texture_sizes.clear();
    }
}

impl<'renderer, 'font> Drop for TextTextureRegistry<'renderer, 'font> {
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

impl<'renderer, 'font> state::TextTextureRegistry for TextTextureRegistry<'renderer, 'font> {
    type Error = FrontendError;

    fn clear(&mut self) {
        self.clear()
    }

    fn update_positions(&mut self, values: impl Iterator<Item = sdl3_sys::SDL_FRect>) {
        let clipped_positions =
            values
                .zip(self.texture_sizes.iter())
                .map(|(rect, texture_size)| sdl3_sys::SDL_FRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.w.min(texture_size.x),
                    h: rect.h.min(texture_size.y),
                });

        let size_before = self.text_positions.len();
        self.text_positions.clear();
        self.text_positions.extend(clipped_positions);
        let size_after = self.text_positions.len();
        assert_eq!(
            size_before, size_after,
            "the update_positions took an iterator which provided unexpected amount of values."
        );
    }

    fn create(
        &mut self,
        text: impl Into<Vec<u8>>,
        color: Color,
        position: sdl3_sys::SDL_FRect,
    ) -> Result<(), Self::Error> {
        unsafe {
            let wrap_length: i32 = {
                let p = position.w.floor();
                assert!(p <= i32::MAX as f32);
                p as i32
            };

            let cstring =
                std::ffi::CString::new(text).map_err(FrontendError::CStringIsNotCreated)?;
            let surf: sdlext::Surface = sdlext::ttf_render_text_blended_wrapped(
                &mut self.font.borrow_mut(),
                cstring.as_c_str(),
                color.into(),
                wrap_length,
            )
            .map_err(FrontendError::TextObjectIsNotRegistered)?;

            let texture: sdlext::Texture =
                sdlext::create_texture_from_surface(self.renderer, &surf)
                    .map_err(FrontendError::TextObjectIsNotRegistered)?;

            let (texture_width, texture_height): (f32, f32) = {
                let mut width = 0f32;
                let mut height = 0f32;
                if !sdl::SDL_GetTextureSize(texture.ptr(), &mut width, &mut height) {
                    panic!("the texture size failed to be obtained");
                }
                (width, height)
            };

            let pos = sdl::SDL_FRect {
                x: position.x,
                y: position.y,
                w: texture_width.min(position.w as _),
                h: texture_height.min(position.h as _),
            };

            self.surfaces.push(surf);
            self.textures.push(texture);
            self.texture_sizes.push(sdl3_sys::SDL_FPoint {
                x: texture_width,
                y: texture_height,
            });
            self.text_positions.push(pos);
        }
        Ok(())
    }
}

struct DumbFrontend<'renderer, 'font> {
    hour_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,
    days_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,
    dates_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,
}

impl<'renderer, 'font> Frontend for DumbFrontend<'renderer, 'font> {
    type TextObject = sdlext::Text;
    type Error = FrontendError;
    type TextTextureRegistry = TextTextureRegistry<'renderer, 'font>;
    type AgendaSource = KhalAgendaSource;

    fn get_hours_text_registry(&mut self) -> &mut TextTextureRegistry<'renderer, 'font> {
        &mut self.hour_text_texture_regirsty
    }

    fn get_days_text_registry(&mut self) -> &mut Self::TextTextureRegistry {
        &mut self.days_text_texture_regirsty
    }

    fn get_dates_text_registry(&mut self) -> &mut Self::TextTextureRegistry {
        &mut self.dates_text_texture_regirsty
    }

    fn get_current_week_start(&self) -> Result<calendar::date::Date, Self::Error> {
        sdlext::get_current_time()
            .and_then(date::get_week_start)
            .map_err(FrontendError::WeekStartIsNotObtained)
    }

    fn agenda_source(&self) -> &Self::AgendaSource {
        &KhalAgendaSource
    }
}

/// It provides the data from the program Khal.  It provides the data according the trait
/// AgendaSource.
struct KhalAgendaSource;

impl AgendaSource for KhalAgendaSource {
    type RequestHandle = *mut sdl::SDL_Process;

    type Error = FrontendError;

    fn request(
        &self,
        week_start: &calendar::date::Date,
    ) -> Result<Self::RequestHandle, Self::Error> {
        let mut arguments = calendar::obtain::khal::week_arguments(week_start);
        let bin: Result<String, _> = std::env::var("SEMANA_BACKEND_BIN");
        if let Ok(ref v) = bin {
            arguments.backend_bin_path = v.as_ref();
        }

        let from = arguments.from.iso_8601();
        unsafe {
            let args: [&str; 18] = [
                arguments.backend_bin_path,
                "list",
                "--json",
                "title",
                "--json",
                "start-date",
                "--json",
                "start-time",
                "--json",
                "end-date",
                "--json",
                "end-time",
                "--json",
                "all-day",
                "--json",
                "calendar-color",
                from.as_str(),
                &format!("{}d", arguments.duration_days),
            ];
            use std::ffi::CString;
            let args_cstrings: Vec<CString> =
                args.iter().map(|s| CString::new(*s).unwrap()).collect();
            let mut args_ptrs: Vec<*const std::ffi::c_char> =
                args_cstrings.iter().map(|cs| cs.as_ptr()).collect();
            args_ptrs.push(std::ptr::null());

            let vec_ptr: *const *const std::ffi::c_char = args_ptrs.as_ptr();
            let args_ptr: *const *const i8 = vec_ptr.cast();
            let ret: *mut sdl::SDL_Process = sdl::SDL_CreateProcess(args_ptr.cast(), true);
            if ret.is_null() {
                Err(FrontendError::AgendaSourceFailed(
                    sdlext::Error::ProcessIsNotCreated,
                ))
            } else {
                Ok(ret)
            }
        }
    }

    fn cancel(&self, handle: &Self::RequestHandle) {
        unsafe {
            let force: bool = true;
            sdl::SDL_KillProcess(*handle, force);
        }
    }

    fn free(&self, handle: Self::RequestHandle) {
        unsafe {
            sdl::SDL_DestroyProcess(handle);
        }
    }

    fn is_ready(&self, handle: &Self::RequestHandle) -> bool {
        let block = false;
        let mut exit_code = 0;
        unsafe { sdl::SDL_WaitProcess(*handle, block, &mut exit_code) }
    }

    fn fetch(
        &self,
        handle: &Self::RequestHandle,
        week_start: &calendar::date::Date,
    ) -> calendar::obtain::WeekScheduleWithLanes {
        let mut exit_code = 0;
        let mut size = 0;
        unsafe {
            let ret: *const std::ffi::c_void =
                sdl::SDL_ReadProcess(*handle, &mut size, &mut exit_code);
            let byte_ptr: *const i8 = ret.cast();
            let output_cstr = std::ffi::CStr::from_ptr(byte_ptr);
            let output_str: &str = output_cstr.to_str().expect("can't convert to utf-8");
            calendar::obtain::parse_events(&calendar::obtain::NanoSerde, output_str, week_start)
                .map(|events| calendar::obtain::get_lanes(events, week_start))
                .expect("fail to parse events")
        }
    }
}

fn unsafe_main() {
    let event_offset = sdl::SDL_FPoint {
        x: 100f32,
        y: 70f32,
    };

    unsafe {
        let ret: Result<(), Error> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: &sdlext::Renderer| {
                let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
                _ = sdl::SDL_GetWindowSize(root_window, &mut window_size.x, &mut window_size.y);

                sdl_ttf_init(
                    renderer,
                    move |_engine: *mut sdl_ttf::TTF_TextEngine| -> Result<(), Error> {
                        let fonts = Fonts::new(config::FONT_PATH, config::FONT_PATH)?;
                        let title_font_height: std::ffi::c_int =
                            sdl_ttf::TTF_GetFontHeight(fonts.title.borrow_mut().ptr());

                        let mut short_event_text_registry =
                            TextTextureRegistry::new(renderer, &fonts.title);
                        let mut long_event_text_registry =
                            TextTextureRegistry::new(renderer, &fonts.title);

                        // hours (00:00, 01:00 etc)
                        let hour_text_texture_regirsty =
                            TextTextureRegistry::new(renderer, &fonts.ui);
                        // days (Monday, Tuesday etc.)
                        let days_text_texture_regirsty =
                            TextTextureRegistry::new(renderer, &fonts.ui);
                        // dates (2025-12-16, 2025-12-17 etc)
                        let dates_text_texture_regirsty =
                            TextTextureRegistry::new(renderer, &fonts.ui);

                        let mut frontend = DumbFrontend {
                            hour_text_texture_regirsty,
                            days_text_texture_regirsty,
                            dates_text_texture_regirsty,
                        };

                        let mut app = App::new(&mut frontend, title_font_height, event_offset)?;
                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        'outer_loop: loop {
                            // stage: event handle
                            while sdl::SDL_PollEvent(&mut event as _) {
                                match event.type_ {
                                    sdl::SDL_EVENT_QUIT => break 'outer_loop,
                                    sdl::SDL_EVENT_WINDOW_RESIZED => {
                                        app.calendar.request_render();
                                        _ = sdl::SDL_GetWindowSize(
                                            root_window,
                                            &mut window_size.x,
                                            &mut window_size.y,
                                        );
                                    }
                                    sdl::SDL_EVENT_KEY_DOWN => match event.key.key {
                                        sdl::SDLK_UP => {
                                            app.ui.add_adjustment(-config::GRID_OFFSET_STEP);
                                            app.calendar.request_render();
                                        }
                                        sdl::SDLK_DOWN => {
                                            app.ui.add_adjustment(config::GRID_OFFSET_STEP);
                                            app.calendar.request_render();
                                        }
                                        sdl::SDLK_MINUS => {
                                            app.ui.adjustment.vertical_scale = 0f32.max(
                                                app.ui.adjustment.vertical_scale
                                                    - config::GRID_SCALE_STEP,
                                            );
                                            app.calendar.request_render();
                                        }
                                        sdl::SDLK_EQUALS => {
                                            app.ui.adjustment.vertical_scale +=
                                                config::GRID_SCALE_STEP;
                                            app.calendar.request_render();
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

                            let data = app.create_render_data(
                                &mut frontend,
                                window_size,
                                &mut long_event_text_registry,
                                &mut short_event_text_registry,
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
