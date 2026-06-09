mod date;
mod error;
mod render;
mod state;

use core::cell::RefCell;
use core::mem::MaybeUninit;

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;

use sdlext::Ptr;
use sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init};

use crate::error::{Error, FrontendError};
use crate::state::{AgendaSource, GetLongEventTextRegistry, GetShortEventTextRegistry};
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
    pub const FONT_CONTENT: &[u8] = include_bytes!("../../../assets/DejaVuSansMonoBook.ttf");
    pub const COLOR_BACKGROUND: u32 = 0x0C0D0C;
    pub const COLOR_TEXT_HIGHLIGHT: u32 = 0x009900;
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
    #[allow(unused)]
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

    fn from_bytes(title_font_buffer: &[u8], ui_font_buffer: &[u8]) -> Result<Self, sdlext::Error> {
        let title_font: RefCell<Font> =
            Font::from_buffer(title_font_buffer, 16.0).map(RefCell::new)?;

        let ui_font: RefCell<Font> = Font::from_buffer(ui_font_buffer, 22.0).map(RefCell::new)?;
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

            let RenderedText { surf, texture } =
                RenderedText::try_new(self.renderer, color, text, self.font, wrap_length)?;

            let (texture_width, texture_height): (f32, f32) = {
                let mut width = 0f32;
                let mut height = 0f32;
                if !sdl::SDL_GetTextureSize(texture.ptr(), &mut width, &mut height) {
                    return Err(FrontendError::TextObjectIsNotRegistered(
                        sdlext::Error::CantGetTextureCoordinates,
                    ));
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

struct RenderedText {
    surf: sdlext::Surface,
    texture: sdlext::Texture,
}

impl RenderedText {
    fn try_new<'renderer>(
        renderer: &'renderer sdlext::Renderer,
        color: Color,
        text: impl Into<Vec<u8>>,
        font: &RefCell<Font>,
        wrap_length: i32,
    ) -> Result<Self, FrontendError> {
        let cstring = std::ffi::CString::new(text).map_err(FrontendError::CStringIsNotCreated)?;
        let surf: sdlext::Surface = sdlext::ttf_render_text_blended_wrapped(
            &mut font.borrow_mut(),
            cstring.as_c_str(),
            color.into(),
            wrap_length,
        )
        .map_err(FrontendError::TextObjectIsNotRegistered)?;

        let texture: sdlext::Texture = sdlext::create_texture_from_surface(renderer, &surf)
            .map_err(FrontendError::TextObjectIsNotRegistered)?;

        Ok(Self { surf, texture })
    }
}

struct TextObjectRegistry<'font> {
    font: &'font RefCell<sdlext::Font>,
    text_engine: *mut sdl_ttf::TTF_TextEngine,
    text_positions: Vec<sdl::SDL_FRect>,
    text_objects: Vec<sdlext::Text>,
}

impl<'font> TextObjectRegistry<'font> {
    fn new(font: &'font RefCell<sdlext::Font>, text_engine: *mut sdl_ttf::TTF_TextEngine) -> Self {
        Self {
            font,
            text_positions: Vec::new(),
            text_engine,
            text_objects: Vec::new(),
        }
    }

    pub fn render(&self) -> Result<(), sdlext::Error> {
        for (text, position) in self.text_objects.iter().zip(self.text_positions.iter()) {
            let sdl::SDL_FRect { x, y, .. } = position;
            unsafe {
                if !sdl_ttf::TTF_DrawRendererText(text.ptr(), *x, *y) {
                    return Err(sdlext::TtfError::TextIsNotDrawn.into());
                }
            }
        }
        Ok(())
    }
}

impl<'font> state::TextObjectRegistry for TextObjectRegistry<'font> {
    type Error = FrontendError;

    type TextObject = sdlext::Text;

    fn clear(&mut self) {
        self.text_objects.clear();
        self.text_positions.clear();
    }

    fn get(&self, index: usize) -> Option<&Self::TextObject> {
        self.text_objects.get(index)
    }

    fn create(
        &mut self,
        text: impl Into<Vec<u8>>,
        position: sdl3_sys::SDL_FRect,
    ) -> Result<(), Self::Error> {
        let cstring = std::ffi::CString::new(text).expect("malware input for a C string");
        let ret = sdlext::Text::try_new(self.text_engine, &mut self.font.borrow_mut(), &cstring)
            .expect("the text object hasn't been created");
        unsafe {
            let width: f32 = position.w.floor();
            if !sdl_ttf::TTF_SetTextWrapWidth(ret.ptr(), width as i32) {
                return Err(FrontendError::TextObjectIsNotRegistered(
                    sdlext::Error::TtfError(sdlext::TtfError::TextCantBeWrapped),
                ));
            }
        }

        self.text_objects.push(ret);
        self.text_positions.push(position);
        Ok(())
    }
}

struct DumbFrontend<'renderer, 'font> {
    text_engine: TextEngine,
    hour_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,
    days_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,
    dates_text_texture_regirsty: TextTextureRegistry<'renderer, 'font>,

    long_event_text_registry: TextTextureRegistry<'renderer, 'font>,
    short_event_text_registry: TextTextureRegistry<'renderer, 'font>,
    event_details_text_object_regirsty: TextObjectRegistry<'font>,
}

impl<'renderer, 'font> GetLongEventTextRegistry for DumbFrontend<'renderer, 'font> {
    type Registry = TextTextureRegistry<'renderer, 'font>;

    fn get_long_event_text_registry(&mut self) -> &mut Self::Registry {
        &mut self.long_event_text_registry
    }
}

impl<'renderer, 'font> GetShortEventTextRegistry for DumbFrontend<'renderer, 'font> {
    type Registry = TextTextureRegistry<'renderer, 'font>;

    fn get_short_event_text_registry(&mut self) -> &mut Self::Registry {
        &mut self.short_event_text_registry
    }
}

struct TextEngine {
    window: *mut sdl::SDL_Window,
}

impl state::TextEngine for TextEngine {
    type TextObject = sdlext::Text;

    type Error = FrontendError;

    /// Returns the offset from the relative the beginning of the TTF_Text based on the `position`.
    ///
    /// SDL_ttf supports calculation of the substring at the given `position`.  The offset is
    /// the offset of the substring relatively to the beginning of TTF_Text.
    fn get_offset(
        &self,
        text_object: &Self::TextObject,
        position: &sdl3_sys::SDL_FPoint,
    ) -> Result<i32, Self::Error> {
        let substring: sdl_ttf::TTF_SubString = unsafe {
            let mut substring: MaybeUninit<sdl_ttf::TTF_SubString> = MaybeUninit::zeroed();
            if !sdl_ttf::TTF_GetTextSubStringForPoint(
                text_object.ptr(),
                position.x as i32,
                position.y as i32,
                substring.as_mut_ptr(),
            ) {
                return Err(FrontendError::CursorClickHandlingFailure(
                    sdlext::Error::TtfError(sdlext::TtfError::NoSubstringForPoint),
                ));
            }

            substring.assume_init()
        };

        Ok(substring.offset)
    }

    /// Returns the rectangles highlighting the text of the given `text_object`.  Each rectangle
    /// corresponds its line of the `text_object` within the given range determined by `start` and
    /// `len`.
    fn calculate_highlights(
        &self,
        text_object: &Self::TextObject,
        start: i32,
        len: i32,
    ) -> Result<Vec<sdl::SDL_FRect>, Self::Error> {
        unsafe {
            let ret: *mut *mut sdl_ttf::TTF_SubString = sdl_ttf::TTF_GetTextSubStringsForRange(
                text_object.ptr(),
                start,
                len,
                core::ptr::null_mut(),
            );
            if ret.is_null() {
                Err(FrontendError::CursorClickHandlingFailure(
                    sdlext::Error::TtfError(sdlext::TtfError::NoSubstringForPoint),
                ))
            } else {
                let mut outvec = Vec::new();
                let mut cursor = 0;
                while !ret.add(cursor).is_null() {
                    let substring: *mut sdl_ttf::TTF_SubString = *ret.add(cursor);
                    if substring.is_null() {
                        break;
                    }

                    let rect: sdl3_ttf_sys::SDL_Rect = (*substring).rect as _;
                    let out = sdl::SDL_FRect {
                        x: rect.x as f32,
                        y: rect.y as f32,
                        w: rect.w as f32,
                        h: rect.h as f32,
                    };
                    outvec.push(out);
                    cursor += 1;
                }
                Ok(outvec)
            }
        }
    }

    fn get_description_cursor_position(
        &self,
        text_object: &Self::TextObject,
        position: &sdl3_sys::SDL_FPoint,
    ) -> Result<sdl3_sys::SDL_FRect, Self::Error> {
        // SAFETY: the input data for the function call is validated.
        let substring: sdl_ttf::TTF_SubString = unsafe {
            let mut substring: MaybeUninit<sdl_ttf::TTF_SubString> = MaybeUninit::zeroed();
            if !sdl_ttf::TTF_GetTextSubStringForPoint(
                text_object.ptr(),
                position.x as i32,
                position.y as i32,
                substring.as_mut_ptr(),
            ) {
                return Err(FrontendError::CursorClickHandlingFailure(
                    sdlext::Error::TtfError(sdlext::TtfError::NoSubstringForPoint),
                ));
            }

            substring.assume_init()
        };

        let cursor_rect = sdl::SDL_FRect {
            x: substring.rect.x as f32,
            y: substring.rect.y as f32,
            w: substring.rect.w as f32,
            h: substring.rect.h as f32,
        };
        Ok(cursor_rect)
    }
}

impl<'renderer, 'font> Frontend for DumbFrontend<'renderer, 'font> {
    type TextObject = sdlext::Text;
    type Error = FrontendError;
    type TextTextureRegistry = TextTextureRegistry<'renderer, 'font>;
    type AgendaSource = KhalAgendaSource;
    type TextObjectRegistry = TextObjectRegistry<'font>;
    type TextEngine = TextEngine;

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

    fn get_event_details_text_object_regirsty(&self) -> &Self::TextObjectRegistry {
        &self.event_details_text_object_regirsty
    }

    fn get_event_details_text_object_regirsty_mut(&mut self) -> &mut Self::TextObjectRegistry {
        &mut self.event_details_text_object_regirsty
    }

    fn get_text_engine(&self) -> &Self::TextEngine {
        &self.text_engine
    }

    fn agenda_source(&self) -> &Self::AgendaSource {
        &KhalAgendaSource
    }

    fn set_clipboard(&self, text: impl Into<Vec<u8>>) -> Result<(), Self::Error> {
        unsafe {
            let cstring =
                std::ffi::CString::new(text).map_err(FrontendError::CStringIsNotCreated)?;
            if !sdl::SDL_SetClipboardText(cstring.as_c_str().as_ptr()) {
                Err(FrontendError::ClipboardIsBroken(
                    sdlext::Error::CantSetClipboard,
                ))
            } else {
                Ok(())
            }
        }
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
            let args: [&str; _] = [
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
                "--json",
                "description",
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

    // FIXME(alex): it's potentially a bomb.  The documentation of SDL_WaitProcess says that all of
    // the output has to be read before calling the function.  Otherwise it can never return true.
    // Currently, it works but it's luck I guess.
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
                // FIXME(alex): this panics if the process provides unsupported input
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
                    move |engine: *mut sdl_ttf::TTF_TextEngine| -> Result<(), Error> {
                        let fonts = Fonts::from_bytes(config::FONT_CONTENT, config::FONT_CONTENT)?;
                        let title_font_height: std::ffi::c_int =
                            sdl_ttf::TTF_GetFontHeight(fonts.title.borrow_mut().ptr());

                        let mouse = sdl::SDL_FPoint { x: 0., y: 0. };
                        let short_event_text_registry =
                            TextTextureRegistry::new(renderer, &fonts.title);
                        let long_event_text_registry =
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
                        let event_details_text_object_regirsty =
                            TextObjectRegistry::new(&fonts.ui, engine);
                        let text_engine = TextEngine {
                            window: root_window,
                        };

                        let mut frontend = DumbFrontend {
                            hour_text_texture_regirsty,
                            days_text_texture_regirsty,
                            dates_text_texture_regirsty,
                            short_event_text_registry,
                            long_event_text_registry,
                            event_details_text_object_regirsty,
                            text_engine,
                        };

                        let event_title_offset = sdl::SDL_FPoint {
                            x: config::EVENT_TITLE_OFFSET_X,
                            y: config::EVENT_TITLE_OFFSET_Y,
                        };

                        let mut app = App::new(
                            &mut frontend,
                            title_font_height,
                            event_offset,
                            mouse,
                            event_title_offset,
                        )?;

                        let mut activity: state::Activity = app.get_root_activity();
                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        'outer_loop: loop {
                            let mut events: Vec<state::Action> = Vec::new();
                            // stage: event handle
                            while sdl::SDL_PollEvent(&mut event as _) {
                                match event.type_ {
                                    sdl::SDL_EVENT_QUIT => break 'outer_loop,
                                    sdl::SDL_EVENT_WINDOW_RESIZED => {
                                        events.push(state::Action::WindowResize);
                                        window_size.x = event.window.data1;
                                        window_size.y = event.window.data2;
                                    }
                                    sdl::SDL_EVENT_KEY_DOWN => match event.key.key {
                                        // FIXME(alex): the key handling should be within the
                                        // application logic rather than platform.
                                        sdl::SDLK_C
                                            if (event.key.mod_ as u32 & sdl::SDL_KMOD_CTRL) > 0 =>
                                        {
                                            events.push(state::Action::Yank);
                                        }
                                        sdl::SDLK_UP => {
                                            events.push(state::Action::Scroll(
                                                -config::GRID_OFFSET_STEP,
                                            ));
                                        }
                                        sdl::SDLK_DOWN => {
                                            events.push(state::Action::Scroll(
                                                config::GRID_OFFSET_STEP,
                                            ));
                                        }
                                        sdl::SDLK_MINUS => {
                                            events.push(state::Action::Zoom(
                                                -config::GRID_SCALE_STEP,
                                            ));
                                        }
                                        sdl::SDLK_EQUALS => {
                                            events
                                                .push(state::Action::Zoom(config::GRID_SCALE_STEP));
                                            app.calendar.request_render();
                                        }
                                        _ => (),
                                    },
                                    sdl::SDL_EVENT_KEY_UP => match event.key.key {
                                        sdl::SDLK_ESCAPE => events.push(state::Action::Escape),
                                        sdl::SDLK_PAGEUP => {
                                            events.push(state::Action::SubtractWeek)
                                        }
                                        sdl::SDLK_PAGEDOWN => events.push(state::Action::AddWeek),
                                        _ => (),
                                    },
                                    sdl::SDL_EVENT_MOUSE_BUTTON_UP => {
                                        events.push(state::Action::MouseButtonUp)
                                    }
                                    sdl::SDL_EVENT_MOUSE_MOTION => {
                                        events.push(state::Action::MouseMove {
                                            x: event.motion.x,
                                            y: event.motion.y,
                                        })
                                    }

                                    sdl::SDL_EVENT_MOUSE_BUTTON_DOWN => {
                                        use state::MouseButton::*;
                                        let button: Option<state::MouseButton> =
                                            match event.button.button {
                                                1 => Some(Left),
                                                2 => Some(Right),
                                                3 => Some(Middle),
                                                4 => Some(Back),
                                                5 => Some(Forth),
                                                _ => None,
                                            };

                                        if let Some(button) = button {
                                            events.push(state::Action::MouseButtonDown {
                                                position: sdl::SDL_FPoint {
                                                    x: event.button.x,
                                                    y: event.button.y,
                                                },
                                                button,
                                            });
                                        }
                                    }
                                    _ => (),
                                }
                            }

                            let new_state = app.create_render_data(
                                activity,
                                &mut frontend,
                                window_size,
                                events.into_iter(),
                            )?;

                            let data = new_state.render_data;
                            activity = new_state.activity;

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
