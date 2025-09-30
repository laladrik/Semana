use sdl3_sys as sdl;

enum SdlError {
    InitError,
    WindowIsNotCreated,
    CannotSetVsync,
    RenderDrawColorIsNotSet,
    RenderIsNotPresent,
    RenderClearFailed,
    TimeError(TimeError),
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

            let mut renderer = Box::from_raw(renderer);
            if !sdl::SDL_SetRenderVSync(renderer.as_mut(), 1) {
                return Err(SdlError::CannotSetVsync);
            }

            let from: String = get_week_start()
                .map(format_date)
                .map_err(SdlError::TimeError)?;
            let mut arguments = calendar::khal::week_arguments(&from);
            let bin: Result<String, _> = std::env::var("SEMANA_BACKEND_BIN");
            arguments.backend_bin_path = match bin {
                Ok(ref v) => v.as_ref(),
                Err(_) => "khal",
            };

            let res: Result<calendar::Agenda, _> =
                calendar::obtain(&calendar::AgendaSourceStd, &calendar::NanoSerde, &arguments);
            match res {
                Ok(agenda) => {
                    println!("agenda {:?}", agenda);
                }
                Err(err) => {
                    println!("failed to obtain the calendar agenda {:?}", err);
                }
            }

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

                set_color(renderer.as_mut(), Color::from_rgb(0xffffff))?;
                if !sdl::SDL_RenderClear(renderer.as_mut()) {
                    return Err(SdlError::RenderClearFailed);
                }

                set_color(renderer.as_mut(), Color::from_rgb(0xdddddd))?;
                let row_ratio: f32 = window_size.y as f32 / 24.0;
                for i in 0..24 {
                    let ordinate = i as f32 * row_ratio;
                    let _ = sdl::SDL_RenderLine(
                        renderer.as_mut(),
                        0.,
                        ordinate,
                        window_size.x as f32,
                        ordinate,
                    );
                }

                let col_ratio: f32 = window_size.x as f32 / 7.;
                for i in 0..7 {
                    let absciss: f32 = i as f32 * col_ratio;
                    _ = sdl::SDL_RenderLine(
                        renderer.as_mut(),
                        absciss,
                        0.,
                        absciss,
                        window_size.y as f32,
                    );
                }

                if !sdl::SDL_RenderPresent(renderer.as_mut()) {
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
            }
        }
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

fn set_color(renderer: &mut sdl::SDL_Renderer, color: Color) -> SdlResult<()> {
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
