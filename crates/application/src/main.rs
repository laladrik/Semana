use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
mod sdlext;

use crate::sdlext::{sdl_init, sdl_ttf_init, set_color, Color, SdlError, SdlFont, SdlResult, TimeError};

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
    unsafe {
        let ret: Result<(), SdlError> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: *mut sdl::SDL_Renderer| {
                let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
                _ = sdl::SDL_GetWindowSize(root_window, &mut window_size.x, &mut window_size.y);

                sdl_ttf_init(renderer, move |engine: *mut sdl_ttf::TTF_TextEngine| {
                    let font_path = c"/home/antlord/.local/share/fonts/DejaVuSansMonoBook.ttf";
                    let mut font = SdlFont::open(font_path, 22.0).map_err(SdlError::from)?;
                    let monday = c"Monday";
                    let color: sdl_ttf::SDL_Color = Color::from_rgb(0xff0000).into();
                    let surface: *mut sdl::SDL_Surface = sdl_ttf::TTF_RenderText_Solid_Wrapped(
                        font.as_mut_ptr(),
                        monday.as_ptr(),
                        monday.count_bytes(),
                        color,
                        0,
                    )
                    .cast();
                    if surface.is_null() {
                        panic!("the surface for the text is not created");
                    }

                    let texture: *mut sdl::SDL_Texture =
                        sdl::SDL_CreateTextureFromSurface(renderer, surface);
                    if texture.is_null() {
                        panic!("the texture for the text is not created");
                    }

                    let destination_rectlangle = sdl::SDL_FRect {
                        x: 200.0,
                        y: 200.0,
                        w: (*surface).w as f32,
                        h: (*surface).h as f32,
                    };

                    let text =
                        sdl_ttf::TTF_CreateText(engine, font.as_mut_ptr(), c"Laladrik".as_ptr(), 8);
                    if text.is_null() {
                        panic!("text is not created");
                    }

                    let (mut r, mut g, mut b, mut a) = (0, 0, 0, 0);
                    if !sdl_ttf::TTF_GetTextColor(text, &mut r, &mut g, &mut b, &mut a) {
                        panic!("can't get text color");
                    } else {
                        println!("text color {} {} {} {}", r, g, b, a);
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

                        set_color(renderer, Color::from_rgb(0x000000))?;
                        if !sdl::SDL_RenderClear(renderer) {
                            return Err(SdlError::RenderClearFailed);
                        }

                        // let col_ratio: f32 = window_size.x as f32 / 7.;
                        // let arguments = calendar::render::Arguments {
                        //     column_width: col_ratio,
                        //     column_height: window_size.y as f32,
                        // };

                        // let render_res: Result<_, _> =
                        //     calendar::render::into_rectangles(&agenda, &arguments);
                        // match render_res {
                        //     Ok(rectangles) => {
                        //         calendar::render::render_rectangles(rectangles.iter(), &rectangle_render)?;
                        //     }
                        //     Err(err) => panic!("fail to turn the events into the rectangles {:?}", err),
                        // }

                        //render_grid(renderer, window_size)?;
                        set_color(renderer, Color::from_rgb(0x111111))?;
                        if !sdl_ttf::TTF_DrawRendererText(text, 100., 100.) {
                            panic!("text is not renderered");
                        }

                        let rect_ptr: *const sdl::SDL_FRect = &destination_rectlangle;
                        if !sdl::SDL_RenderTexture(
                            renderer,
                            texture,
                            std::ptr::null(),
                            rect_ptr.cast(),
                        ) {
                            panic!("fail to render the texture for the text");
                        }

                        if !sdl::SDL_RenderPresent(renderer) {
                            return Err(SdlError::RenderIsNotPresent);
                        }
                    }

                    let _ = root_window;
                    Ok(())
                })
            },
        );

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
                SdlError::RectangleIsNotDrawn => todo!("handle rectangle render errors"),
                SdlError::TtfError(_) => todo!("handle SDL TTF error"),
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
