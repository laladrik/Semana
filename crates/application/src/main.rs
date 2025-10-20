use std::{cell::RefCell, mem::MaybeUninit};

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
mod sdlext;

use crate::sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init, set_color};

#[inline(always)]
fn format_date(date: &calendar::Date) -> String {
    format!("{}-{:02}-{:02}", date.year, date.month, date.day)
}

fn get_current_week_start() -> Result<calendar::Date, TimeError> {
    sdlext::get_current_time().and_then(date::get_week_start)
}

struct SdlTextCreate<'a> {
    engine: *mut sdl_ttf::TTF_TextEngine,
    font: &'a RefCell<sdlext::Font>,
}

impl calendar::TextCreate for SdlTextCreate<'_> {
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

struct TextRegistry {
    surfaces: Vec<*mut sdl::SDL_Surface>,
    textures: Vec<*mut sdl::SDL_Texture>,
    text_positions: Vec<sdl::SDL_FRect>,
    renderer: *mut sdl::SDL_Renderer,
}

impl TextRegistry {
    fn new(renderer: *mut sdl::SDL_Renderer) -> Self {
        Self {
            surfaces: Vec::new(),
            textures: Vec::new(),
            text_positions: Vec::new(),
            renderer,
        }
    }

    fn create(
        &mut self,
        text: &std::ffi::CStr,
        font: &RefCell<Font>,
        position: sdl::SDL_FRect,
    ) -> Result<(), sdlext::Error> {
        unsafe {
            let wrap_length: i32 = {
                let p = position.w.floor();
                assert!(p <= i32::MAX as f32);
                p as i32
            };

            let surf: *mut sdl_ttf::SDL_Surface = sdl_ttf::TTF_RenderText_Blended_Wrapped(
                font.borrow_mut().as_mut_ptr(),
                text.as_ptr(),
                text.count_bytes(),
                Color::from_rgb(0xffffff).into(),
                wrap_length,
            );

            if surf.is_null() {
                return Err(sdlext::Error::SurfaceIsNotCreated);
            }

            let texture = sdl::SDL_CreateTextureFromSurface(self.renderer, surf.cast());
            if texture.is_null() {
                sdl::SDL_DestroySurface(surf.cast());
                return Err(sdlext::Error::TextureIsNotCreated);
            }

            self.surfaces.push(surf.cast());
            self.textures.push(texture);
            let pos = {
                let mut texture_width = 0f32;
                let mut texture_height = 0f32;
                let _ = sdl::SDL_GetTextureSize(texture, &mut texture_width, &mut texture_height);
                sdl::SDL_FRect {
                    x: position.x,
                    y: position.y,
                    w: texture_width.min(position.w as _),
                    h: texture_height.min(position.h as _),
                }
            };

            self.text_positions.push(pos);
        }
        Ok(())
    }

    fn render(&self) -> Result<(), sdlext::Error> {
        for (texture, position) in self.textures.iter().zip(self.text_positions.iter()) {
            unsafe {
                let src = sdl::SDL_FRect {
                    x: 0f32,
                    y: 0f32,
                    w: position.w,
                    h: position.h,
                };

                if !sdl::SDL_RenderTexture(self.renderer, *texture, &src, position) {
                    return Err(sdlext::Error::TextureIsNotRendered);
                }
            }
        }
        Ok(())
    }
    fn clear(&mut self) {
        unsafe {
            while let Some(ptr) = self.surfaces.pop() {
                sdl::SDL_DestroySurface(ptr);
            }

            while let Some(ptr) = self.textures.pop() {
                sdl::SDL_DestroyTexture(ptr);
            }

            self.text_positions.clear();
        }
    }
}

impl Drop for TextRegistry {
    fn drop(&mut self) {
        self.clear()
    }
}

struct Fonts {
    title: RefCell<Font>,
    ui: RefCell<Font>,
}

impl Fonts {
    fn new(
        title_font_path: &std::ffi::CStr,
        ui_font_path: &std::ffi::CStr,
    ) -> Result<Self, sdlext::Error> {
        let title_font: RefCell<Font> = Font::open(title_font_path, 16.0).map(RefCell::new)?;

        let ui_font: RefCell<Font> = Font::open(ui_font_path, 22.0).map(RefCell::new)?;
        Ok(Self {
            title: title_font,
            ui: ui_font,
        })
    }
}

#[derive(Debug)]
struct CalendarError {
    data: String,
}

impl<'event> From<calendar::Error<'event>> for CalendarError {
    fn from(value: calendar::Error<'event>) -> Self {
        let (calendar::Error::InvalidDate(data) | calendar::Error::InvalidTime(data)) = value;
        Self {
            data: data.to_owned(),
        }
    }
}

#[derive(Debug)]
enum Error {
    Sdl(sdlext::Error),
    Calendar(CalendarError),
}

impl From<sdlext::Error> for Error {
    fn from(value: sdlext::Error) -> Self {
        Error::Sdl(value)
    }
}

impl From<CalendarError> for Error {
    fn from(value: CalendarError) -> Self {
        Error::Calendar(value)
    }
}

fn register_event_titles<'rect, 'event: 'rect, Str>(
    text_registry: &mut TextRegistry,
    font: &RefCell<Font>,
    titles: &[Str],
    rectangles: &'rect [calendar::render::Rectangle<'event>],
) -> Result<(), Error>
where
    Str: AsRef<str>,
{
    assert_eq!(titles.len(), rectangles.len());
    for item in titles.iter().zip(rectangles.iter()) {
        let (title, rectangle): (&Str, &calendar::render::Rectangle) = item;
        let c_title =
            std::ffi::CString::new(title.as_ref()).expect("can't create c string for an event");
        let offset_x = 2f32;
        let offset_y = 4f32;
        let dstrect = sdl::SDL_FRect {
            x: rectangle.at.x + offset_x,
            y: rectangle.at.y + offset_y,
            w: rectangle.size.x - offset_x * 2f32,
            h: rectangle.size.y - offset_y * 2f32,
        };

        text_registry.create(c_title.as_c_str(), font, dstrect)?;
    }
    Ok(())
}

fn unsafe_main() {
    let font_path = c"/home/antlord/.local/share/fonts/DejaVuSansMonoBook.ttf";
    unsafe {
        let ret: Result<(), Error> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: *mut sdl::SDL_Renderer| {
                let mut text_registry = TextRegistry::new(renderer);
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

                sdl_ttf_init(
                    renderer,
                    move |engine: *mut sdl_ttf::TTF_TextEngine| -> Result<_, Error> {
                        let fonts = Fonts::new(font_path, font_path)?;
                        let ui_text_factory = SdlTextCreate {
                            engine,
                            font: &fonts.ui,
                        };

                        let today: calendar::Date =
                            date::get_today().map_err(sdlext::Error::from)?;
                        let stream = calendar::DateStream::new(today).take(7);
                        let week: calendar::ui::Week<Result<sdlext::Text, _>> =
                            calendar::ui::create_texts(&ui_text_factory, stream);
                        let week: Week = validate_week(week)?;
                        let event_render = RectangleRender { renderer };

                        let week_start: calendar::Date =
                            get_current_week_start().map_err(sdlext::Error::from)?;
                        let week_start_string: String = format_date(&week_start);
                        let mut arguments =
                            calendar::obtain::khal::week_arguments(&week_start_string);
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

                        let event_titles: Vec<&str> = agenda
                            .iter()
                            .filter(|x| x.all_day == "False")
                            .map(|x| x.title.as_str())
                            .collect();
                        let all_day_event_titles: Vec<&str> = agenda
                            .iter()
                            .filter(|x| x.all_day == "True")
                            .map(|x| x.title.as_str())
                            .collect();

                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        let mut scrollable_rectangles: Option<calendar::render::Rectangles> = None;
                        let mut pinned_rectangles_opt: Option<calendar::render::Rectangles> = None;

                        'outer_loop: loop {
                            while sdl::SDL_PollEvent(&mut event as _) {
                                if event.type_ == sdl::SDL_EVENT_QUIT {
                                    break 'outer_loop;
                                }

                                if event.type_ == sdl::SDL_EVENT_WINDOW_RESIZED {
                                    pinned_rectangles_opt.take();
                                    scrollable_rectangles.take();
                                    _ = sdl::SDL_GetWindowSize(
                                        root_window,
                                        &mut window_size.x,
                                        &mut window_size.y,
                                    );
                                }
                            }

                            let top_panel_height = event_surface_rectangle.h / 25.;
                            let cell_width: f32 = event_surface_rectangle.w / 7.;

                            let pinned_rectangles: &calendar::render::Rectangles = {
                                let create = || {
                                    let arguments = calendar::render::Arguments {
                                        column_width: cell_width,
                                        column_height: top_panel_height,
                                        offset_x: event_surface_rectangle.x,
                                        offset_y: event_surface_rectangle.y,
                                    };
                                    let pinned_rectangles_res: Result<
                                        calendar::render::Rectangles,
                                        calendar::Error,
                                    > = calendar::render::whole_day_rectangles(
                                        &agenda,
                                        &week_start,
                                        &arguments,
                                    );

                                    pinned_rectangles_res
                                };

                                let ret: Result<&calendar::render::Rectangles, CalendarError> =
                                    match pinned_rectangles_opt {
                                        Some(ref x) => Ok(x),
                                        None => {
                                            let replacement =
                                                create().map_err(CalendarError::from)?;
                                            // TODO: implement a facility which creates the titles
                                            // of the events at once for the "All day" events and
                                            // regular events.  This would allow to prevent
                                            // accidential calling of `clear` twice.
                                            text_registry.clear();
                                            register_event_titles(
                                                &mut text_registry,
                                                &fonts.title,
                                                &all_day_event_titles,
                                                &replacement,
                                            )?;
                                            Ok(pinned_rectangles_opt.get_or_insert(replacement))
                                        }
                                    };

                                ret?
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
                                return Err(sdlext::Error::RenderClearFailed)?;
                            }

                            let create_rectangles = || {
                                let rectangles: calendar::render::Rectangles = {
                                    let arguments = calendar::render::Arguments {
                                        column_width: grid_rectangle.w / 7.,
                                        column_height: grid_rectangle.h,
                                        offset_x: grid_rectangle.x,
                                        offset_y: grid_rectangle.y,
                                    };

                                    let scroll_rectangles_res: Result<
                                        calendar::render::Rectangles,
                                        _,
                                    > = calendar::render::event_rectangles(
                                        &agenda,
                                        &week_start,
                                        &arguments,
                                    );
                                    scroll_rectangles_res
                                        .expect("fail to turn the events into the rectangles")
                                };
                                rectangles
                            };

                            if scrollable_rectangles.is_none() {
                                let new_rectangles = create_rectangles();
                                register_event_titles(
                                    &mut text_registry,
                                    &fonts.title,
                                    &event_titles,
                                    &new_rectangles,
                                )?;
                                scrollable_rectangles.replace(new_rectangles);
                            }

                            let rectangles = scrollable_rectangles.as_ref().unwrap();
                            calendar::render::render_rectangles(
                                pinned_rectangles.iter(),
                                &event_render,
                            )?;

                            calendar::render::render_rectangles(rectangles.iter(), &event_render)?;
                            render_grid(renderer, &grid_rectangle)?;
                            text_registry.render()?;
                            set_color(renderer, Color::from_rgb(0x111111))?;
                            let render_week_captions_args =
                                calendar::render::RenderWeekCaptionsArgs {
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
                            .collect::<Result<(), sdlext::TtfError>>()
                            .map_err(sdlext::Error::from)?;

                            if !sdl::SDL_RenderPresent(renderer) {
                                return Err(sdlext::Error::RenderIsNotPresent)?;
                            }
                        }

                        let _ = root_window;
                        Ok(())
                    },
                )
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
) -> Result<(), sdlext::Error> {
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

impl calendar::render::RenderRectangles for RectangleRender {
    type Result = Result<(), sdlext::Error>;

    fn render_rectangles<'r, 's: 'r, I>(&self, rectangles: I) -> Self::Result
    where
        I: Iterator<Item = &'r calendar::render::Rectangle<'s>>,
    {
        set_color(self.renderer, Color::from_rgb(0x9999ff))?;
        let data = Vec::from_iter(rectangles.map(create_sdl_frect));
        unsafe {
            if !sdl::SDL_RenderFillRects(self.renderer, data.as_ptr(), data.len() as i32) {
                return Err(sdlext::Error::RectangleIsNotDrawn);
            }

            for rect in data.iter() {
                let border = sdl::SDL_FRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.w,
                    h: 5.0,
                };

                set_color(self.renderer, Color::from_rgb(0xff0000))?;
                if !sdl::SDL_RenderFillRect(self.renderer, &border) {
                    return Err(sdlext::Error::RectangleIsNotDrawn);
                }
            }
        }
        Ok(())
    }
}

fn create_sdl_frect(from: &calendar::render::Rectangle<'_>) -> sdl::SDL_FRect {
    sdl::SDL_FRect {
        x: from.at.x,
        y: from.at.y,
        w: from.size.x,
        h: from.size.y,
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
