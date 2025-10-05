use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl3_ttf;

enum SdlError {
    InitError,
    WindowIsNotCreated,
    CannotSetVsync,
    RenderDrawColorIsNotSet,
    RenderIsNotPresent,
    RenderClearFailed,
    TimeError(TimeError),
    RectangleIsNotDrawn,
}

type SdlResult<R> = Result<R, SdlError>;

fn sdl_init<R>(body: impl Fn() -> SdlResult<R>) -> SdlResult<R> {
    unsafe {
        if !sdl::SDL_Init(sdl::SDL_INIT_VIDEO) {
            return Err(SdlError::InitError);
        }
    }

    let r = body();
    unsafe {
        sdl::SDL_Quit();
    }
    r
}

enum TimeError {
    FailGettingNow,
    FailConvertingNowToDate,
}

#[inline(always)]
fn format_date(date: (u16, u8, u8)) -> String {
    format!("{}-{:02}-{:02}", date.0, date.1, date.2)
}

fn get_week_start() -> Result<(u16, u8, u8), TimeError> {
    unsafe {
        let mut now: sdl::SDL_Time = 0;
        if !sdl::SDL_GetCurrentTime(&mut now as *mut _) {
            return Err(TimeError::FailGettingNow);
        }

        let mut today: sdl::SDL_DateTime = std::mem::zeroed();
        let local_time = true;
        if !sdl::SDL_TimeToDateTime(now, &mut today as *mut _, local_time) {
            return Err(TimeError::FailConvertingNowToDate);
        }

        //let sunday: std::ffi::c_int = 0;
        // from 0 to 6
        let current_weekday: std::ffi::c_int =
            sdl::SDL_GetDayOfWeek(today.year, today.month, today.day);
        //let mut week_offset: std::ffi::c_int = current_weekday - sunday;

        // from 1 to 7
        let natural_weekday: i32 = current_weekday + 7 * ((7 - current_weekday) / 7);
        // TODO: implement for the case Sunday is the first day.
        // monday => 0
        // ...
        // sunday => -6
        let week_offset_days: i32 = -(natural_weekday - 1);
        const NANOSECONDS_PER_DAY: i64 = (sdl::SDL_NS_PER_SECOND as i64) * 60 * 60 * 24;
        let offset_ns: i64 = (week_offset_days as i64) * NANOSECONDS_PER_DAY;
        let mut first_day_of_week: sdl::SDL_DateTime = std::mem::zeroed();
        if !sdl::SDL_TimeToDateTime(
            now - offset_ns,
            &mut first_day_of_week as *mut _,
            local_time,
        ) {
            return Err(TimeError::FailConvertingNowToDate);
        }

        Ok((
            first_day_of_week.year as u16,
            first_day_of_week.month as u8,
            first_day_of_week.day as u8,
        ))
    }
}

fn unsafe_main() {
    let window_title = std::ffi::CString::from(c"semana");
    unsafe {
        let ret: Result<(), SdlError> = sdl_init(move || {
            let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
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
                return Err(SdlError::WindowIsNotCreated);
            }

            let rectangle_render = RectangleRender { renderer };
            if !sdl::SDL_SetRenderVSync(renderer, 1) {
                return Err(SdlError::CannotSetVsync);
            }

            let from: String = get_week_start()
                .map(format_date)
                .map_err(SdlError::TimeError)?;
            let mut arguments = calendar::obtain::khal::week_arguments(&from);
            let bin: Result<String, _> = std::env::var("SEMANA_BACKEND_BIN");
            arguments.backend_bin_path = match bin {
                Ok(ref v) => v.as_ref(),
                Err(_) => "khal",
            };

            let res: Result<calendar::obtain::Agenda, _> = calendar::obtain::obtain(
                &calendar::obtain::AgendaSourceStd,
                &calendar::obtain::NanoSerde,
                &arguments,
            );

            let agenda = match res {
                Ok(agenda) => agenda,
                Err(err) => panic!("can't get the agenda: {:?}", err),
            };

            let mut event: sdl::SDL_Event = std::mem::zeroed();
            'outer_loop: loop {
                while sdl::SDL_PollEvent(&mut event as _) {
                    if event.type_ == sdl::SDL_EVENT_QUIT {
                        break 'outer_loop;
                    }

                    if event.type_ == sdl::SDL_EVENT_WINDOW_RESIZED {
                        _ = sdl::SDL_GetWindowSize(
                            root_window,
                            &mut window_size.x,
                            &mut window_size.y,
                        );
                    }
                }

                set_color(renderer, Color::from_rgb(0xffffff))?;
                if !sdl::SDL_RenderClear(renderer) {
                    return Err(SdlError::RenderClearFailed);
                }

                let col_ratio: f32 = window_size.x as f32 / 7.;
                let arguments = calendar::render::Arguments {
                    column_width: col_ratio,
                    column_height: window_size.y as f32,
                };

                let render_res: Result<_, _> =
                    calendar::render::into_rectangles(&agenda, &arguments);
                match render_res {
                    Ok(rectangles) => {
                        calendar::render::render_rectangles(rectangles.iter(), &rectangle_render)?;
                    }
                    Err(err) => panic!("fail to turn the events into the rectangles {:?}", err),
                }

                render_grid(renderer, window_size)?;
                if !sdl::SDL_RenderPresent(renderer) {
                    return Err(SdlError::RenderIsNotPresent);
                }
            }

            let _ = root_window;
            Ok(())
        });

        if let Err(err) = ret {
            let err_text = std::ffi::CStr::from_ptr(sdl::SDL_GetError());
            match err {
                SdlError::RenderClearFailed => {
                    println!("RenderClearFailed: {:?}", err_text);
                }
                SdlError::RenderIsNotPresent => {
                    println!("RenderIsNotPresent: {:?}", err_text);
                }
                SdlError::RenderDrawColorIsNotSet => {
                    println!("RenderDrawColorIsNotSet: {:?}", err_text);
                }
                SdlError::WindowIsNotCreated => {
                    println!("WindowIsNotCreated: {:?}", err_text);
                }
                SdlError::CannotSetVsync => {
                    println!("CannotSetVsync");
                }
                SdlError::InitError => {
                    println!("failed to initialize")
                }
                SdlError::TimeError(_) => {
                    println!("failed to process date and time");
                }
                SdlError::RectangleIsNotDrawn => todo!(),
            }
        }
    }
}

fn render_grid(
    renderer: *mut sdl::SDL_Renderer,
    window_size: sdl::SDL_Point,
) -> Result<(), SdlError> {
    unsafe {
        set_color(renderer, Color::from_rgb(0x333333))?;
        let row_ratio: f32 = window_size.y as f32 / 24.0;
        for i in 0..24 {
            let ordinate = i as f32 * row_ratio;
            let _ = sdl::SDL_RenderLine(renderer, 0., ordinate, window_size.x as f32, ordinate);
        }

        let col_ratio: f32 = window_size.x as f32 / 7.;
        for i in 0..7 {
            let absciss: f32 = i as f32 * col_ratio;
            _ = sdl::SDL_RenderLine(renderer, absciss, 0., absciss, window_size.y as f32);
        }
    }
    Ok(())
}

struct RectangleRender {
    renderer: *mut sdl::SDL_Renderer,
}

fn create_sdl_frect(from: &calendar::render::Rectange<'_>) -> sdl::SDL_FRect {
    sdl::SDL_FRect {
        x: from.at.x,
        y: from.at.y,
        w: from.size.x,
        h: from.size.y,
    }
}

impl calendar::render::RenderRectangles for RectangleRender {
    type Result = Result<(), SdlError>;

    fn render_rectangles<'r, 's: 'r, I>(&self, rectangles: I) -> Self::Result
    where
        I: Iterator<Item = &'r calendar::render::Rectange<'s>>,
    {
        set_color(self.renderer, Color::from_rgb(0x9999ff))?;
        let data = Vec::from_iter(rectangles.map(create_sdl_frect));
        unsafe {
            if !sdl::SDL_RenderFillRects(self.renderer, data.as_ptr(), data.len() as i32) {
                return Err(SdlError::RectangleIsNotDrawn);
            }
        }
        Ok(())
    }
}

struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    const fn from_rgb(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: (value) as u8,
            a: 0xff,
        }
    }
}

fn set_color(renderer: *mut sdl::SDL_Renderer, color: Color) -> SdlResult<()> {
    unsafe {
        if !sdl::SDL_SetRenderDrawColor(renderer, color.r, color.g, color.b, color.a) {
            Err(SdlError::RenderDrawColorIsNotSet)
        } else {
            Ok(())
        }
    }
}

fn main() {
    unsafe_main();
}
#[cfg(test)]
mod tests {
    #[test]
    fn div() {
        assert_eq!(0, 6 / 7);
    }
}
