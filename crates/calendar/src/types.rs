#[cfg_attr(test, derive(PartialEq, Debug))]
#[repr(C)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[cfg(feature = "backend-native")]
#[repr(C)]
pub struct FPoint {
    pub x: f32,
    pub y: f32,
}

#[cfg(feature = "backend-native")]
#[repr(C)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[cfg(feature = "backend-sdl3")]
pub type FPoint = sdl3_sys::SDL_FPoint;

#[cfg(feature = "backend-sdl3")]
pub type FRect = sdl3_sys::SDL_FRect;

pub type Size = Point;
