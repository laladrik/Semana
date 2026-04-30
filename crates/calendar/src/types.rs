#[cfg(not(feature = "sdl3-geometry"))]
#[repr(C)]
pub struct FPoint {
    pub x: f32,
    pub y: f32,
}

#[cfg(not(feature = "sdl3-geometry"))]
#[repr(C)]
pub struct FRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[cfg(feature = "sdl3-geometry")]
pub type FPoint = sdl3_sys::SDL_FPoint;

#[cfg(feature = "sdl3-geometry")]
pub type FRect = sdl3_sys::SDL_FRect;

pub type FSize = FPoint;
