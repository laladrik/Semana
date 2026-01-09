use std::ptr::NonNull;

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
#[derive(Debug)]
pub enum Error {
    Init,
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
    SurfaceIsNotScaled,
    RenderTargetFailed,
    RenderFailed,
    ViewportIsNotSet,
    RendererIsNull,
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

pub type Result<R> = std::result::Result<R, Error>;

pub struct Font {
    ptr: NonNull<sdl_ttf::TTF_Font>,
}

impl Ptr for Font {
    type Inner = sdl_ttf::TTF_Font;

    fn ptr(&self) -> *mut Self::Inner {
         self.ptr.as_ptr()
    }
}

impl Font {
    pub fn new(ptr: NonNull<sdl_ttf::TTF_Font>) -> Self {
        Self { ptr }
    }

    pub fn open(path: &std::ffi::CStr, size: f32) -> std::result::Result<Self, TtfError> {
        let ptr = unsafe { sdl_ttf::TTF_OpenFont(path.as_ptr(), size) };
        NonNull::new(ptr)
            .ok_or(TtfError::FontIsNotOpened)
            .map(Self::new)
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
    renderer: &Renderer,
    body: impl FnOnce(*mut sdl_ttf::TTF_TextEngine) -> std::result::Result<R, E>,
) -> std::result::Result<R, E>
where
    E: From<Error>,
{
    unsafe {
        if !sdl_ttf::TTF_Init() {
            panic!("ttf is not initialized");
        }

        let engine: *mut sdl_ttf::TTF_TextEngine =
            sdl_ttf::TTF_CreateRendererTextEngine(renderer.ptr().cast());
        if engine.is_null() {
            return Err(Error::from(TtfError::EngineIsNotCreated))?;
        }

        let r = body(engine);
        sdl_ttf::TTF_DestroyRendererTextEngine(engine);
        r
    }
}

/// # Safety
///
/// It's safe to call the function as long the window is not destroyed in the body.
pub unsafe fn sdl_init<R, E>(
    body: impl FnOnce(*mut sdl::SDL_Window, &Renderer) -> std::result::Result<R, E>,
) -> std::result::Result<R, E>
where
    E: From<Error>,
{
    unsafe {
        if !sdl::SDL_Init(sdl::SDL_INIT_VIDEO) {
            return Err(Error::Init)?;
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

        let mut safe_renderer = Renderer {
            ptr: NonNull::new(renderer).ok_or(Error::RendererIsNull)?,
        };

        let r = body(root_window, &mut safe_renderer);
        sdl::SDL_Quit();
        r
    }
}

pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const GREEN: Self = Self {
        r: 0x00,
        g: 0xff,
        b: 0x00,
        a: 0xff,
    };
    pub const RED: Self = Self {
        r: 0xff,
        g: 0x00,
        b: 0x00,
        a: 0xff,
    };

    pub const fn from_rgb(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: (value) as u8,
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

impl Ptr for &Renderer {
    type Inner = sdl::SDL_Renderer;

    fn ptr(&self) -> *mut Self::Inner {
         self.ptr.as_ptr()
    }
}

pub fn set_color(renderer: &Renderer, color: Color) -> Result<()> {
    unsafe {
        if !sdl::SDL_SetRenderDrawColor(renderer.ptr(), color.r, color.g, color.b, color.a) {
            Err(Error::RenderDrawColorIsNotSet)
        } else {
            Ok(())
        }
    }
}

pub struct Text {
    ptr: *mut sdl_ttf::TTF_Text,
}

impl Ptr for Text {
    type Inner = sdl_ttf::TTF_Text;

    fn ptr(&self) -> *mut Self::Inner {
        self.ptr
    }
}

impl Text {
    pub fn try_new(
        engine: *mut sdl_ttf::TTF_TextEngine,
        font: &mut Font,
        text: &std::ffi::CStr,
    ) -> std::result::Result<Self, TtfError> {
        unsafe {
            let ptr =
                sdl_ttf::TTF_CreateText(engine, font.ptr(), text.as_ptr(), text.count_bytes());
            if ptr.is_null() {
                Err(TtfError::TextIsNotCreated)
            } else {
                Ok(Self { ptr })
            }
        }
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        unsafe {
            sdl_ttf::TTF_DestroyText(self.ptr);
        }
    }
}

pub fn get_current_time() -> std::result::Result<sdl::SDL_Time, TimeError> {
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
) -> std::result::Result<sdl::SDL_DateTime, TimeError> {
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

#[derive(Clone, Copy)]
pub enum ScaleMode {
    Invalid,
    Nearest,
    Linear,
}

impl From<ScaleMode> for sdl::SDL_ScaleMode {
    fn from(value: ScaleMode) -> Self {
        match value {
            ScaleMode::Invalid => sdl::SDL_ScaleMode_SDL_SCALEMODE_INVALID,
            ScaleMode::Nearest => sdl::SDL_ScaleMode_SDL_SCALEMODE_NEAREST,
            ScaleMode::Linear => sdl::SDL_ScaleMode_SDL_SCALEMODE_LINEAR,
        }
    }
}

impl Surface {
    pub fn new(ptr: NonNull<sdl::SDL_Surface>) -> Self {
        Self { ptr }
    }

    pub fn create_rgb24(w: i32, h: i32) -> Result<Self> {
        unsafe {
            NonNull::new(sdl::SDL_CreateSurface(
                w,
                h,
                sdl::SDL_PixelFormat_SDL_PIXELFORMAT_RGB24,
            ))
            .ok_or(Error::SurfaceIsNotCreated)
            .map(Self::new)
        }
    }

    pub fn scale(&mut self, w: i32, h: i32, mode: ScaleMode) -> Result<()> {
        unsafe {
            NonNull::new(sdl::SDL_ScaleSurface(
                self.ptr.as_mut(),
                w,
                h,
                sdl::SDL_ScaleMode::from(mode),
            ))
            .ok_or(Error::SurfaceIsNotScaled)
            .map(|ptr| {
                self.ptr = ptr;
            })
        }
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
) -> Result<Surface> {
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

    pub fn create_rgb25(renderer: &Renderer, w: i32, h: i32) -> Result<Texture> {
        unsafe {
            let format = sdl::SDL_PixelFormat_SDL_PIXELFORMAT_RGB24;
            let access = sdl::SDL_TextureAccess_SDL_TEXTUREACCESS_TARGET;
            let t = sdl::SDL_CreateTexture(renderer.ptr(), format, access, w, h);
            NonNull::new(t)
                .ok_or(Error::TextureIsNotCreated)
                .map(Texture::new)
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        // SAFETY the code is safe as long as the safety of [`Texture::as_ptr`] is considered.
        unsafe { sdl::SDL_DestroyTexture(self.ptr.as_ptr()) }
    }
}

pub fn create_texture_from_surface(
    renderer: &Renderer,
    surface: &Surface,
) -> Result<Texture> {
    // SAFETY: the calling of the function is safe because the pointers of renderer and surface are
    // guaranteed to be valid because they are validated during the creation of the instances and
    // don't change during their life.
    //
    // TODO: consider guarantee calling the function from the main thread only.
    unsafe {
        let texture = sdl::SDL_CreateTextureFromSurface(renderer.ptr(), surface.ptr());
        NonNull::new(texture)
            .ok_or(Error::TextureIsNotCreated)
            .map(Texture::new)
    }
}

pub trait Ptr {
    type Inner;
    fn ptr(&self) -> *mut Self::Inner;
}

impl Ptr for Surface {
    type Inner = sdl::SDL_Surface;

    fn ptr(&self) -> *mut Self::Inner {
         self.ptr.as_ptr()
    }
}

impl Ptr for Texture {
    type Inner = sdl::SDL_Texture;

    fn ptr(&self) -> *mut Self::Inner {
         self.ptr.as_ptr()
    }
}

fn to_mut_ptr<'a, U, T>(reference: impl Into<Option<&'a mut T>>) -> *mut U
where
    T: Ptr<Inner = U> + 'a,
{
    reference
        .into()
        .map(|x| x.ptr())
        .unwrap_or(std::ptr::null_mut())
}

pub fn set_render_target<'a>(
    renderer: &Renderer,
    event_sdl_texture: impl Into<Option<&'a mut Texture>>,
) -> Result<()> {
    unsafe {
        if !sdl::SDL_SetRenderTarget(renderer.ptr(), to_mut_ptr(event_sdl_texture)) {
            Err(Error::RenderTargetFailed)
        } else {
            Ok(())
        }
    }
}

pub fn render_clear(renderer: &mut sdl::SDL_Renderer) -> Result<()> {
    unsafe {
        if !sdl::SDL_RenderClear(renderer) {
            Err(Error::RenderClearFailed)
        } else {
            Ok(())
        }
    }
}

pub fn render_rect(renderer: &mut sdl::SDL_Renderer, rect: sdl::SDL_FRect) -> Result<()> {
    unsafe {
        if !sdl::SDL_RenderRect(renderer, &rect as _) {
            Err(Error::RenderFailed)
        } else {
            Ok(())
        }
    }
}

pub fn render_fill_rect(
    renderer: &mut sdl::SDL_Renderer,
    rect: sdl::SDL_FRect,
) -> Result<()> {
    unsafe {
        if !sdl::SDL_RenderFillRect(renderer, &rect as _) {
            Err(Error::RenderFailed)
        } else {
            Ok(())
        }
    }
}

pub struct Renderer {
    ptr: NonNull<sdl::SDL_Renderer>,
}

impl Renderer {
    pub fn ptr(&self) -> *mut sdl::SDL_Renderer {
         self.ptr.as_ptr()
    }
}

pub fn set_render_viewport<'a>(
    renderer: &Renderer,
    rect: impl Into<Option<&'a sdl::SDL_Rect>>,
) -> Result<()> {
    unsafe {
        let ptr: *const sdl::SDL_Rect = rect
            .into()
            .map(|r| r as *const _)
            .unwrap_or(std::ptr::null());
        if !sdl::SDL_SetRenderViewport(renderer.ptr(), ptr) {
            Err(Error::ViewportIsNotSet)
        } else {
            Ok(())
        }
    }
}

pub fn set_render_clip_rect<'a>(
    renderer: &mut sdl::SDL_Renderer,
    rect: impl Into<Option<&'a sdl::SDL_Rect>>,
) -> Result<()> {
    unsafe {
        let ptr: *const sdl::SDL_Rect = rect
            .into()
            .map(|r| r as *const _)
            .unwrap_or(std::ptr::null());
        if !sdl::SDL_SetRenderClipRect(renderer, ptr) {
            Err(Error::ViewportIsNotSet)
        } else {
            Ok(())
        }
    }
}


//pub type Result<T> = Result<T, Error>;
