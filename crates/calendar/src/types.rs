#[cfg(not(feature = "sdl3-geometry"))]
#[repr(C)]
pub struct Rest {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[cfg(not(feature = "sdl3-geometry"))]
#[repr(C)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

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
pub type Point = sdl3_sys::SDL_Point;

#[cfg(feature = "sdl3-geometry")]
pub type FRect = sdl3_sys::SDL_FRect;

#[cfg(feature = "sdl3-geometry")]
pub type Rect = sdl3_sys::SDL_Rect;

pub type FSize = FPoint;

#[inline]
pub fn is_fpoint_between_points(
    point: impl core::borrow::Borrow<FPoint>,
    left_top: impl core::borrow::Borrow<FPoint>,
    bottom_right: impl core::borrow::Borrow<FPoint>,
) -> bool {
    let FPoint { x, y } = point.borrow();
    let FPoint { x: lx, y: ly } = left_top.borrow();
    let FPoint { x: rx, y: ry } = bottom_right.borrow();
    x > lx && y > ly && x < rx && y < ry
}

pub fn fpoint_add(left: impl core::borrow::Borrow<FPoint>, x: f32, y: f32) -> FPoint {
    let left: &FPoint = left.borrow();
    FPoint {
        x: left.x + x,
        y: left.y + y,
    }
}

pub fn fpoint_sub(left: impl core::borrow::Borrow<FPoint>, x: f32, y: f32) -> FPoint {
    fpoint_add(left, -x, -y)
}

pub trait MoveFRect {
    fn move_frect(self, x: f32, y: f32) -> Self;
}

impl MoveFRect for FRect {
    fn move_frect(self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            h: self.h,
            w: self.w,
        }
    }
}

pub trait AddFPoint {
    fn add_fpoint(self, x: f32, y: f32) -> FPoint;
}

pub trait CoversPoint {
    fn covers_point(&self, _: &FPoint) -> bool;
}

impl CoversPoint for FRect {
    fn covers_point(&self, point: &FPoint) -> bool {
        (point.x >= self.x)
            && (point.x <= (self.x + self.w))
            && (point.y >= self.y)
            && (point.y <= (self.y + self.h))
    }
}

impl<T: core::borrow::Borrow<FPoint>> AddFPoint for T {
    fn add_fpoint(self, x: f32, y: f32) -> FPoint {
        fpoint_add(self, x, y)
    }
}

pub trait SubFPoint {
    fn sub_fpoint(self, x: f32, y: f32) -> FPoint;
}

impl<T: core::borrow::Borrow<FPoint>> SubFPoint for T {
    fn sub_fpoint(self, x: f32, y: f32) -> FPoint {
        fpoint_sub(self, x, y)
    }
}

pub trait AsFPoint {
    fn as_fpoint(&self) -> FPoint;
}

impl AsFPoint for Point {
    fn as_fpoint(&self) -> FPoint {
        FPoint {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

pub trait AsRect {
    fn as_rect(&self) -> Rect;
}

impl AsRect for FRect {
    fn as_rect(&self) -> Rect {
        Rect {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            w: self.w.floor() as i32,
            h: self.h.floor() as i32,
        }
    }
}
