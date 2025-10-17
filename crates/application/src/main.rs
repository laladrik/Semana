use std::{cell::RefCell, mem::MaybeUninit};

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
mod sdlext;

use crate::sdlext::{Color, Error, Font, TimeError, sdl_init, sdl_ttf_init, set_color};

#[inline(always)]
fn format_date(date: (u16, u8, u8)) -> String {
    format!("{}-{:02}-{:02}", date.0, date.1, date.2)
}

fn get_current_week_start() -> Result<(u16, u8, u8), TimeError> {
    sdlext::get_current_time().and_then(date::get_week_start)
}

struct SdlTextCreate {
    engine: *mut sdl_ttf::TTF_TextEngine,
    font: RefCell<sdlext::Font>,
}

impl calendar::TextCreate for SdlTextCreate {
    type Result = Result<sdlext::Text, sdlext::TtfError>;

    fn text_create(&self, s: &str) -> Self::Result {
        let cstring = std::ffi::CString::new(s).unwrap();
        sdlext::Text::try_new(self.engine, &mut self.font.borrow_mut(), cstring.as_c_str())
    }
}

struct SdlTextRender;

impl calendar::render::TextRender for SdlTextRender {
    type Text = sdlext::Text;

    type Result = Result<(), sdlext::TtfError>;

    fn text_render(&self, text: &Self::Text, x: f32, y: f32) -> Self::Result {
        unsafe {
            if !sdl_ttf::TTF_DrawRendererText(text.ptr().get(), x, y) {
                Err(sdlext::TtfError::TextIsNotDrown)
            } else {
                Ok(())
            }
        }
    }
}

mod date {
    use super::{TimeError, sdl, sdlext};

    pub fn get_today() -> Result<calendar::Date, TimeError> {
        let now = sdlext::get_current_time()?;
        let today = sdlext::time_to_date_time(now, true)?;
        Ok(calendar::Date {
            day: today.day as u8,
            month: today.month as u8,
            year: today.year as u16,
        })
    }

    pub fn get_week_start(now: sdl::SDL_Time) -> Result<(u16, u8, u8), TimeError> {
        let local_time = true;
        unsafe {
            let today = sdlext::time_to_date_time(now, local_time)?;
            //let sunday: std::ffi::c_int = 0;
            // from 0 to 6
            let current_weekday: std::ffi::c_int =
                sdl::SDL_GetDayOfWeek(today.year, today.month, today.day);
            //let mut week_offset: std::ffi::c_int = current_weekday - sunday;

            let natural_weekday: i32 = match current_weekday {
                0 => 7,
                d @ 1..7 => d,
                _ => panic!("SDL has returned an invalid week day"),
            };
            // from 1 to 7
            //let natural_weekday: i32 = current_weekday + 7 * ((7 - current_weekday) / 7);
            // TODO: implement for the case Sunday is the first day.
            // monday => 0
            // ...
            // sunday => -6
            let week_offset_days: i32 = -(natural_weekday - 1);
            const NANOSECONDS_PER_DAY: i64 = (sdl::SDL_NS_PER_SECOND as i64) * 60 * 60 * 24;
            let offset_ns: i64 = (week_offset_days as i64) * NANOSECONDS_PER_DAY;
            let mut first_day_of_week: sdl::SDL_DateTime = std::mem::zeroed();
            if !sdl::SDL_TimeToDateTime(
                now + offset_ns,
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
}

type MaybeText = Result<sdlext::Text, sdlext::TtfError>;
fn validate_array<const N: usize>(
    array: [MaybeText; N],
) -> Result<[sdlext::Text; N], sdlext::TtfError> {
    unsafe {
        let mut out: MaybeUninit<[sdlext::Text; N]> = MaybeUninit::uninit();
        let ptr: *mut sdlext::Text = out.as_mut_ptr() as *mut _;
        for (i, elem) in array.into_iter().enumerate() {
            // SAFETY: the index can't go beyond the array boundaries, because `array` has the same
            // size as `out`.
            ptr.add(i).write(elem?);
        }

        Ok(out.assume_init())
    }
}

type Week = calendar::ui::Week<sdlext::Text>;

fn validate_week(
    dirty: calendar::ui::Week<Result<sdlext::Text, sdlext::TtfError>>,
) -> Result<Week, sdlext::Error> {
    Ok(Week {
        days: validate_array(dirty.days)?,
        hours: validate_array(dirty.hours)?,
        dates: validate_array(dirty.dates)?,
    })
}

fn unsafe_main() {
    let font_path = c"/home/antlord/.local/share/fonts/DejaVuSansMonoBook.ttf";
    unsafe {
        let ret: Result<(), Error> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: *mut sdl::SDL_Renderer| {
                let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
                _ = sdl::SDL_GetWindowSize(root_window, &mut window_size.x, &mut window_size.y);

                let grid_offset = sdl::SDL_FPoint { x: 100., y: 70. };
                sdl_ttf_init(renderer, move |engine: *mut sdl_ttf::TTF_TextEngine| {
                    let font: RefCell<Font> = Font::open(font_path, 22.0)
                        .map_err(Error::from)
                        .map(RefCell::new)?;

                    let text_factory = SdlTextCreate { engine, font };
                    let today: calendar::Date = date::get_today()?;
                    let stream = calendar::DateStream::new(today).take(7);
                    let week: calendar::ui::Week<Result<sdlext::Text, _>> =
                        calendar::ui::create_texts(&text_factory, stream);
                    let week: Week = validate_week(week)?;
                    let event_render = RectangleRender { renderer };

                    let from: String = get_current_week_start()
                        .map(format_date)
                        .map_err(Error::TimeError)?;
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
                    let event_titles: Vec<sdlext::Text> = {
                        let titles = agenda.iter().map(|x| x.title.as_str());
                        calendar::ui::create_event_title_texts(&text_factory, titles)
                            .collect::<Result<Vec<sdlext::Text>, sdlext::TtfError>>()?
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

                        let grid_size = sdl::SDL_FPoint {
                            x: window_size.x as f32 - grid_offset.x,
                            y: window_size.y as f32 - grid_offset.y,
                        };

                        set_color(renderer, Color::from_rgb(0x000000))?;
                        if !sdl::SDL_RenderClear(renderer) {
                            return Err(Error::RenderClearFailed);
                        }

                        let col_ratio: f32 = grid_size.x / 7.;
                        let row_ratio: f32 = grid_size.y / 24.;
                        let arguments = calendar::render::Arguments {
                            column_width: col_ratio,
                            column_height: grid_size.y,
                            offset_x: grid_offset.x,
                            offset_y: grid_offset.y,
                        };

                        let render_res: Result<calendar::render::Rectangles, _> =
                            calendar::render::event_rectangles(&agenda, &arguments);
                        match render_res {
                            Ok(rectangles) => {
                                let items = rectangles.iter().zip(event_titles.iter()).map(
                                    |(rectangle, title)| calendar::render::EventText {
                                        text: title,
                                        at: calendar::render::Point {
                                            x: rectangle.at.x + 2.0,
                                            y: rectangle.at.y + 2.0,
                                        },
                                    },
                                );

                                calendar::render::render_rectangles(
                                    rectangles.iter(),
                                    &event_render,
                                )?;
                                calendar::render::event_texts(&SdlTextRender, items)
                                    .collect::<Result<Vec<_>, _>>()?;
                            }
                            Err(err) => {
                                panic!("fail to turn the events into the rectangles {:?}", err)
                            }
                        }

                        render_grid(renderer, grid_size, grid_offset)?;
                        set_color(renderer, Color::from_rgb(0x111111))?;
                        let render_week_captions_args = calendar::render::RenderWeekCaptionsArgs {
                            hours_arguments: calendar::render::RenderHoursArgs {
                                row_height: row_ratio,
                                offset_x: 10.,
                                offset_y: grid_offset.y + 5.,
                            },
                            days_arguments: calendar::render::Arguments {
                                column_width: col_ratio,
                                column_height: 0.,
                                offset_x: grid_offset.x,
                                offset_y: 10.0,
                            },
                            dates_arguments: calendar::render::Arguments {
                                column_width: col_ratio,
                                column_height: 0.,
                                offset_x: grid_offset.x,
                                offset_y: 35.0,
                            },
                        };

                        calendar::render::render_week_captions(
                            &SdlTextRender,
                            week.days.iter(),
                            week.hours.iter(),
                            week.dates.iter(),
                            &render_week_captions_args,
                        )
                        .collect::<Result<(), sdlext::TtfError>>()?;

                        if !sdl::SDL_RenderPresent(renderer) {
                            return Err(Error::RenderIsNotPresent);
                        }
                    }

                    let _ = root_window;
                    Ok(())
                })
            },
        );

        if let Err(err) = ret {
            println!("The application failed with the error {:?}", err);
        }
    }
}

fn render_grid(
    renderer: *mut sdl::SDL_Renderer,
    size: sdl::SDL_FPoint,
    offset: sdl::SDL_FPoint,
) -> Result<(), Error> {
    unsafe {
        set_color(renderer, Color::from_rgb(0x333333))?;
        let row_ratio: f32 = size.y / 24.0;
        for i in 0..24 {
            let ordinate = i as f32 * row_ratio + offset.y;
            let _ = sdl::SDL_RenderLine(renderer, offset.x, ordinate, size.x + offset.x, ordinate);
        }

        let col_ratio: f32 = size.x / 7.;
        for i in 0..7 {
            let absciss: f32 = i as f32 * col_ratio + offset.x;
            _ = sdl::SDL_RenderLine(renderer, absciss, offset.y, absciss, size.y + offset.y);
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
    type Result = Result<(), Error>;

    fn render_rectangles<'r, 's: 'r, I>(&self, rectangles: I) -> Self::Result
    where
        I: Iterator<Item = &'r calendar::render::Rectange<'s>>,
    {
        set_color(self.renderer, Color::from_rgb(0x9999ff))?;
        let data = Vec::from_iter(rectangles.map(create_sdl_frect));
        unsafe {
            if !sdl::SDL_RenderFillRects(self.renderer, data.as_ptr(), data.len() as i32) {
                return Err(Error::RectangleIsNotDrawn);
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
            let res =
                get_week_start(now_time).expect("getting the start of the week must not fail");
            let (year, month, day) = res;
            assert_eq!(2025, year);
            assert_eq!(10, month);
            assert_eq!(6, day);
        }
    }
}
