use std::{cell::RefCell, mem::MaybeUninit};

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
mod sdlext;

use crate::sdlext::{Color, Error, Font, TimeError, sdl_init, sdl_ttf_init, set_color};

#[inline(always)]
fn format_date(date: &calendar::Date) -> String {
    format!("{}-{:02}-{:02}", date.year, date.month, date.day)
}

fn get_current_week_start() -> Result<calendar::Date, TimeError> {
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

mod date;

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

                // Event surface contains the grid with the events and the top panel with the
                // "All Day" events.
                let event_surface_rectangle: sdl::SDL_FRect = {
                    let x = 100.;
                    let y = 70.;
                    sdl::SDL_FRect {
                        x,
                        y,
                        h: window_size.y as f32 - y,
                        w: window_size.x as f32 - x,
                    }
                };

                sdl_ttf_init(renderer, move |engine: *mut sdl_ttf::TTF_TextEngine| {
                    let title_font: RefCell<Font> = Font::open(font_path, 16.0)
                        .map_err(Error::from)
                        .map(RefCell::new)?;

                    let ui_font: RefCell<Font> = Font::open(font_path, 22.0)
                        .map_err(Error::from)
                        .map(RefCell::new)?;

                    let ui_text_factory = SdlTextCreate {
                        engine,
                        font: ui_font,
                    };
                    let title_text_factory = SdlTextCreate {
                        engine,
                        font: title_font,
                    };
                    let today: calendar::Date = date::get_today()?;
                    let stream = calendar::DateStream::new(today).take(7);
                    let week: calendar::ui::Week<Result<sdlext::Text, _>> =
                        calendar::ui::create_texts(&ui_text_factory, stream);
                    let week: Week = validate_week(week)?;
                    let event_render = RectangleRender { renderer };

                    let week_start: calendar::Date = get_current_week_start()?;
                    let week_start_string: String = format_date(&week_start);
                    let mut arguments = calendar::obtain::khal::week_arguments(&week_start_string);
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
                        let titles = agenda
                            .iter()
                            .filter(|x| x.all_day == "False")
                            .map(|x| x.title.as_str());
                        calendar::ui::create_event_title_texts(&title_text_factory, titles)
                            .collect::<Result<Vec<sdlext::Text>, sdlext::TtfError>>()?
                    };

                    let pinned_event_titles: Vec<sdlext::Text> = {
                        let titles = agenda
                            .iter()
                            .filter(|x| x.all_day == "True")
                            .map(|x| x.title.as_str());
                        calendar::ui::create_event_title_texts(&title_text_factory, titles)
                            .collect::<Result<Vec<sdlext::Text>, sdlext::TtfError>>()?
                    };
                    let mut event: sdl::SDL_Event = std::mem::zeroed();
                    let mut rectangles: Option<calendar::render::Rectangles> = None;
                    'outer_loop: loop {
                        let mut recalculate_events_rectangles = false;
                        while sdl::SDL_PollEvent(&mut event as _) {
                            if event.type_ == sdl::SDL_EVENT_QUIT {
                                break 'outer_loop;
                            }

                            if event.type_ == sdl::SDL_EVENT_WINDOW_RESIZED {
                                recalculate_events_rectangles = true;
                                _ = sdl::SDL_GetWindowSize(
                                    root_window,
                                    &mut window_size.x,
                                    &mut window_size.y,
                                );
                            }
                        }

                        let top_panel_height = event_surface_rectangle.h / 25.;
                        let cell_width: f32 = event_surface_rectangle.w / 7.;
                        let pinned_rectangles: calendar::render::Rectangles = {
                            let arguments = calendar::render::Arguments {
                                column_width: cell_width,
                                column_height: top_panel_height,
                                offset_x: event_surface_rectangle.x,
                                offset_y: event_surface_rectangle.y,
                            };
                            let pinned_rectangles_res: Result<calendar::render::Rectangles, _> =
                                calendar::render::whole_day_rectangles(
                                    &agenda,
                                    &week_start,
                                    &arguments,
                                );

                            pinned_rectangles_res.expect("the input for the rectangles is invalid")
                        };

                        let grid_vertical_offset = if pinned_rectangles.is_empty() {
                            0f32
                        } else {
                            top_panel_height
                        };

                        let grid_rectangle = sdl::SDL_FRect {
                            x: event_surface_rectangle.x,
                            y: event_surface_rectangle.y + grid_vertical_offset,
                            w: event_surface_rectangle.w,
                            h: event_surface_rectangle.h - grid_vertical_offset,
                        };

                        let cell_height = if pinned_rectangles.is_empty() {
                            grid_rectangle.h / 24.
                        } else {
                            top_panel_height
                        };

                        set_color(renderer, Color::from_rgb(0x000000))?;
                        if !sdl::SDL_RenderClear(renderer) {
                            return Err(Error::RenderClearFailed);
                        }

                        let create_rectangles = || {
                            let rectangles: calendar::render::Rectangles = {
                                let arguments = calendar::render::Arguments {
                                    column_width: grid_rectangle.w / 7.,
                                    column_height: grid_rectangle.h,
                                    offset_x: grid_rectangle.x,
                                    offset_y: grid_rectangle.y,
                                };

                                let scroll_rectangles_res: Result<calendar::render::Rectangles, _> =
                                    calendar::render::event_rectangles(
                                        &agenda,
                                        &week_start,
                                        &arguments,
                                    );
                                scroll_rectangles_res
                                    .expect("fail to turn the events into the rectangles")
                            };
                            rectangles
                        };

                        if recalculate_events_rectangles || rectangles.is_none() {
                            rectangles.replace(create_rectangles());
                        }

                        let rectangles = rectangles.as_ref().unwrap();

                        let event_texts =
                            calendar::render::place_event_texts(rectangles, &event_titles);
                        let pinned_event_texts = calendar::render::place_event_texts(
                            &pinned_rectangles,
                            &pinned_event_titles,
                        );

                        calendar::render::render_rectangles(
                            pinned_rectangles.iter(),
                            &event_render,
                        )?;
                        calendar::render::render_rectangles(rectangles.iter(), &event_render)?;
                        calendar::render::event_texts(&SdlTextRender, event_texts)
                            .collect::<Result<Vec<_>, _>>()?;
                        calendar::render::event_texts(&SdlTextRender, pinned_event_texts)
                            .collect::<Result<Vec<_>, _>>()?;

                        render_grid(renderer, &grid_rectangle)?;
                        set_color(renderer, Color::from_rgb(0x111111))?;
                        let render_week_captions_args = calendar::render::RenderWeekCaptionsArgs {
                            hours_arguments: calendar::render::RenderHoursArgs {
                                row_height: cell_height,
                                offset_x: 10.,
                                offset_y: grid_rectangle.y + 5.,
                            },
                            days_arguments: calendar::render::Arguments {
                                column_width: cell_width,
                                column_height: 0.,
                                offset_x: event_surface_rectangle.x,
                                offset_y: 10.0,
                            },
                            dates_arguments: calendar::render::Arguments {
                                column_width: cell_width,
                                column_height: 0.,
                                offset_x: event_surface_rectangle.x,
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
    grid_rectangle: &sdl::SDL_FRect,
) -> Result<(), Error> {
    unsafe {
        set_color(renderer, Color::from_rgb(0x333333))?;
        let row_ratio: f32 = grid_rectangle.h / 24.0;
        for i in 0..24 {
            let ordinate = i as f32 * row_ratio + grid_rectangle.y;
            let _ = sdl::SDL_RenderLine(
                renderer,
                grid_rectangle.x,
                ordinate,
                grid_rectangle.w + grid_rectangle.x,
                ordinate,
            );
        }

        let col_ratio: f32 = grid_rectangle.w / 7.;
        for i in 0..7 {
            let absciss: f32 = i as f32 * col_ratio + grid_rectangle.x;
            _ = sdl::SDL_RenderLine(
                renderer,
                absciss,
                grid_rectangle.y,
                absciss,
                grid_rectangle.h + grid_rectangle.y,
            );
        }
    }
    Ok(())
}

struct RectangleRender {
    renderer: *mut sdl::SDL_Renderer,
}

fn create_sdl_frect(from: &calendar::render::Rectangle<'_>) -> sdl::SDL_FRect {
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
        I: Iterator<Item = &'r calendar::render::Rectangle<'s>>,
    {
        set_color(self.renderer, Color::from_rgb(0x9999ff))?;
        let data = Vec::from_iter(rectangles.map(create_sdl_frect));
        unsafe {
            // if !sdl::SDL_RenderFillRects(self.renderer, data.as_ptr(), data.len() as i32) {
            //     return Err(Error::RectangleIsNotDrawn);
            // }

            for rect in data.iter() {
                set_color(self.renderer, Color::from_rgb(0x9999ff))?;
                if !sdl::SDL_RenderFillRect(self.renderer, rect) {
                    return Err(Error::RectangleIsNotDrawn);
                }

                let border = sdl::SDL_FRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.w,
                    h: 5.0,
                };

                set_color(self.renderer, Color::from_rgb(0xff0000))?;
                if !sdl::SDL_RenderFillRect(self.renderer, &border) {
                    return Err(Error::RectangleIsNotDrawn);
                }
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
            let res = date::get_week_start(now_time)
                .expect("getting the start of the week must not fail");
            let calendar::Date { year, month, day } = res;
            assert_eq!(2025, year);
            assert_eq!(10, month);
            assert_eq!(6, day);
        }
    }
}
