use std::{cell::Cell, ptr::NonNull};

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
#[derive(Debug)]
pub enum Error {
    InitError,
    WindowIsNotCreated,
    CannotSetVsync,
    RenderDrawColorIsNotSet,
    RenderIsNotPresent,
    RenderClearFailed,
    TimeError(TimeError),
    RectangleIsNotDrawn,
    TtfError(TtfError),
    SurfaceIsNotCreated,
    TextureIsNotCreated,
    TextureIsNotRendered,
}

#[derive(Debug)]
pub enum TimeError {
    FailGettingNow,
    FailConvertingNowToDate,
}

impl From<TtfError> for Error {
    fn from(value: TtfError) -> Self {
        Error::TtfError(value)
    }
}

impl From<TimeError> for Error {
    fn from(value: TimeError) -> Self {
        Error::TimeError(value)
    }
}

#[derive(Debug)]
pub enum TtfError {
    FontIsNotOpened,
    TextIsNotCreated,
    EngineIsNotCreated,
    TextIsNotDrown,
}

pub type SdlResult<R> = Result<R, Error>;

pub struct Font {
    ptr: NonNull<sdl_ttf::TTF_Font>,
}

impl Font {
    pub fn new(ptr: NonNull<sdl_ttf::TTF_Font>) -> Self {
        Self { ptr }
    }

    pub fn open(path: &std::ffi::CStr, size: f32) -> Result<Self, TtfError> {
        let ptr = unsafe { sdl_ttf::TTF_OpenFont(path.as_ptr(), size) };
        NonNull::new(ptr)
            .ok_or(TtfError::FontIsNotOpened)
            .map(Self::new)
    }

    pub fn ptr(&mut self) -> *mut sdl_ttf::TTF_Font {
        self.ptr.as_ptr()
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            sdl_ttf::TTF_CloseFont(self.ptr.as_ptr());
        }
    }
}

pub unsafe fn sdl_ttf_init<R, E>(
    renderer: *mut sdl::SDL_Renderer,
    body: impl FnOnce(*mut sdl_ttf::TTF_TextEngine) -> Result<R, E>,
) -> Result<R, E>
where
    E: From<Error>,
{
    unsafe {
        if !sdl_ttf::TTF_Init() {
            panic!("ttf is not initialized");
        }

        let engine: *mut sdl_ttf::TTF_TextEngine =
            sdl_ttf::TTF_CreateRendererTextEngine(renderer.cast());
        if engine.is_null() {
            return Err(Error::from(TtfError::EngineIsNotCreated))?;
        }

        let r = body(engine);
        sdl_ttf::TTF_DestroyRendererTextEngine(engine);
        r
    }
}

pub unsafe fn sdl_init<R, E>(
    body: impl FnOnce(*mut sdl::SDL_Window, *mut sdl::SDL_Renderer) -> Result<R, E>,
) -> Result<R, E>
where
    E: From<Error>,
{
    unsafe {
        if !sdl::SDL_Init(sdl::SDL_INIT_VIDEO) {
            return Err(Error::InitError)?;
        }

        let window_title = std::ffi::CString::from(c"semana");
        let window_size = sdl::SDL_Point { x: 800, y: 600 };
        let mut root_window: *mut sdl::SDL_Window = std::ptr::null_mut();
        let mut renderer: *mut sdl::SDL_Renderer = std::ptr::null_mut();
        let window_flags: sdl::SDL_WindowFlags = sdl::SDL_WINDOW_RESIZABLE;
        if !sdl::SDL_CreateWindowAndRenderer(
            window_title.as_ptr(),
            window_size.x,
            window_size.y,
            window_flags,
            &mut root_window as *mut *mut _,
            &mut renderer as *mut *mut _,
        ) {
            return Err(Error::WindowIsNotCreated)?;
        }

        if !sdl::SDL_SetRenderVSync(renderer, 1) {
            return Err(Error::CannotSetVsync)?;
        }

        let r = body(root_window, renderer);
        sdl::SDL_Quit();
        r
    }
}

pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    pub const fn from_rgb(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: (value) as u8,
            a: 0xff,
        }
    }
}

impl From<calendar::Color> for Color {
    fn from(value: calendar::Color) -> Self {
        let value = u32::from(value);
        Self {
            r: (value >> 24) as u8,
            g: (value >> 16) as u8,
            b: (value >> 8) as u8,
            a: 0xff,
        }
    }
}

impl From<Color> for sdl_ttf::SDL_Color {
    fn from(value: Color) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

pub fn set_color(renderer: *mut sdl::SDL_Renderer, color: Color) -> SdlResult<()> {
    unsafe {
        if !sdl::SDL_SetRenderDrawColor(renderer, color.r, color.g, color.b, color.a) {
            Err(Error::RenderDrawColorIsNotSet)
        } else {
            Ok(())
        }
    }
}

pub struct Text {
    ptr: Cell<*mut sdl_ttf::TTF_Text>,
}

impl Text {
    pub fn try_new(
        engine: *mut sdl_ttf::TTF_TextEngine,
        font: &mut Font,
        text: &std::ffi::CStr,
    ) -> Result<Self, TtfError> {
        unsafe {
            let ptr =
                sdl_ttf::TTF_CreateText(engine, font.ptr(), text.as_ptr(), text.count_bytes());
            if ptr.is_null() {
                Err(TtfError::TextIsNotCreated)
            } else {
                Ok(Self {
                    ptr: Cell::new(ptr),
                })
            }
        }
    }

    // # Safety:
    //
    // It's safe to call the method unsell the value of the pointer is not changed.
    pub unsafe fn ptr(&self) -> Cell<*mut sdl_ttf::TTF_Text> {
        self.ptr.clone()
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        unsafe {
            sdl_ttf::TTF_DestroyText(self.ptr.get());
        }
    }
}

pub fn get_current_time() -> Result<sdl::SDL_Time, TimeError> {
    unsafe {
        let mut now: sdl::SDL_Time = 0;
        if !sdl::SDL_GetCurrentTime(&mut now as *mut _) {
            Err(TimeError::FailGettingNow)
        } else {
            Ok(now)
        }
    }
}

pub fn time_to_date_time(
    ticks: sdl::SDL_Time,
    local_time: bool,
) -> Result<sdl::SDL_DateTime, TimeError> {
    unsafe {
        let mut ret: sdl::SDL_DateTime = std::mem::zeroed();
        if !sdl::SDL_TimeToDateTime(ticks, &mut ret as *mut _, local_time) {
            Err(TimeError::FailConvertingNowToDate)
        } else {
            Ok(ret)
        }
    }
}

pub struct Surface {
    ptr: NonNull<sdl::SDL_Surface>,
}

impl Surface {
    pub fn new(ptr: NonNull<sdl::SDL_Surface>) -> Self {
        Self { ptr }
    }

    // # Safety:
    //
    // It's safe to call the method unsell the value of the pointer is not changed.
    pub unsafe fn ptr(&self) -> *mut sdl::SDL_Surface {
        self.ptr.as_ptr()
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY the code is safe as long as the safety of [`Surface::as_ptr`] is considered.
        unsafe {
            sdl::SDL_DestroySurface(self.ptr.as_ptr());
        }
    }
}

pub fn ttf_render_text_blended_wrapped(
    font: &mut Font,
    text: &std::ffi::CStr,
    color: sdl_ttf::SDL_Color,
    wrap_length: i32,
) -> Result<Surface, Error> {
    unsafe {
        let ptr: *mut sdl_ttf::SDL_Surface = sdl_ttf::TTF_RenderText_Blended_Wrapped(
            font.ptr(),
            text.as_ptr(),
            text.count_bytes(),
            color,
            wrap_length,
        );

        let p: *mut sdl::SDL_Surface = ptr.cast();
        NonNull::new(p)
            .ok_or(Error::SurfaceIsNotCreated)
            .map(Surface::new)
    }
}

pub struct Texture {
    ptr: NonNull<sdl::SDL_Texture>,
}

impl Texture {
    pub fn new(ptr: NonNull<sdl::SDL_Texture>) -> Self {
        Self { ptr }
    }

    /// # Safety
    ///
    /// The method is safe unless the value of the pointer is changed.
    pub unsafe fn ptr(&self) -> *mut sdl::SDL_Texture {
        self.ptr.as_ptr()
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        // SAFETY the code is safe as long as the safety of [`Texture::as_ptr`] is considered.
        unsafe { sdl::SDL_DestroyTexture(self.ptr.as_ptr()) }
    }
}

pub fn create_texture_from_surface(
    renderer: *mut sdl::SDL_Renderer,
    surface: &Surface,
) -> Result<Texture, Error> {
    unsafe {
        let texture = sdl::SDL_CreateTextureFromSurface(renderer, surface.ptr());
        NonNull::new(texture)
            .ok_or(Error::TextureIsNotCreated)
            .map(Texture::new)
    }
}
