use std::{cell::RefCell, mem::MaybeUninit};

use sdl3_sys as sdl;
use sdl3_ttf_sys as sdl_ttf;
mod sdlext;
use calendar::ui::View;

use crate::sdlext::{Color, Font, TimeError, sdl_init, sdl_ttf_init, set_color};

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
    surfaces: Vec<sdlext::Surface>,
    textures: Vec<sdlext::Texture>,
    text_positions: Vec<sdl::SDL_FRect>,
    renderer: *mut sdl::SDL_Renderer,
}

mod config {
    pub const EVENT_TITLE_OFFSET_X: f32 = 2.0;
    pub const EVENT_TITLE_OFFSET_Y: f32 = 4.0;
    pub static FONT_PATH: &std::ffi::CStr = c"assets/DejaVuSansMonoBook.ttf";
    pub const COLOR_BACKGROUND: u32 = 0x0C0D0C;
    pub const COLOR_EVENT_TITLE: u32 = 0x000000;
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

            let surf: sdlext::Surface = sdlext::ttf_render_text_blended_wrapped(
                &mut font.borrow_mut(),
                text,
                Color::from_rgb(config::COLOR_EVENT_TITLE).into(),
                wrap_length,
            )?;

            let texture: sdlext::Texture =
                sdlext::create_texture_from_surface(self.renderer, &surf)?;

            let pos = {
                let (texture_width, texture_height): (f32, f32) = {
                    let mut width = 0f32;
                    let mut height = 0f32;
                    if !sdl::SDL_GetTextureSize(texture.ptr(), &mut width, &mut height) {
                        panic!("the texture size failed to be obtained");
                    }
                    (width, height)
                };

                sdl::SDL_FRect {
                    x: position.x,
                    y: position.y,
                    w: texture_width.min(position.w as _),
                    h: texture_height.min(position.h as _),
                }
            };

            self.surfaces.push(surf);
            self.textures.push(texture);
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

                if !sdl::SDL_RenderTexture(self.renderer, texture.ptr(), &src, position) {
                    return Err(sdlext::Error::TextureIsNotRendered);
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.surfaces.clear();
        self.textures.clear();
        self.text_positions.clear();
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

type JsonParseError = <calendar::obtain::NanoSerde as calendar::obtain::JsonParser>::Error;
type AgendaObtainError = calendar::obtain::Error<JsonParseError>;

#[derive(Debug)]
enum Error {
    Sdl(sdlext::Error),
    Calendar(CalendarError),
    DataIsNotAvailable(AgendaObtainError),
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

fn register_event_titles<Str>(
    text_registry: &mut TextRegistry,
    font: &RefCell<Font>,
    titles: &[Str],
    rectangles: &[calendar::render::Rectangle],
) -> Result<(), Error>
where
    Str: AsRef<str>,
{
    assert_eq!(titles.len(), rectangles.len());
    for item in titles.iter().zip(rectangles.iter()) {
        let (title, rectangle): (&Str, &calendar::render::Rectangle) = item;
        let c_title =
            std::ffi::CString::new(title.as_ref()).expect("can't create c string for an event");
        let offset_x = config::EVENT_TITLE_OFFSET_X;
        let offset_y = config::EVENT_TITLE_OFFSET_Y;
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

fn obtain_agenda(
    week_start: &calendar::Date,
) -> Result<calendar::obtain::WeekScheduleWithLanes, AgendaObtainError> {
    let mut arguments = calendar::obtain::khal::week_arguments(week_start);
    let bin: Result<String, _> = std::env::var("SEMANA_BACKEND_BIN");
    if let Ok(ref v) = bin {
        arguments.backend_bin_path = v.as_ref();
    }

    calendar::obtain::events_with_lanes(
        &calendar::obtain::EventSourceStd,
        &calendar::obtain::NanoSerde,
        &arguments,
    )
}

fn create_short_event_rectangles(
    grid_rectangle: &sdl::SDL_FRect,
    short_events: &calendar::EventData,
    week_start: &calendar::Date,
) -> calendar::render::Rectangles {
    let arguments = calendar::render::Arguments {
        column_width: grid_rectangle.w / 7.,
        column_height: grid_rectangle.h,
        offset_x: grid_rectangle.x,
        offset_y: grid_rectangle.y,
    };

    calendar::render::short_event_rectangles(short_events, week_start, &arguments).collect()
}

fn create_long_event_rectangles<'ev>(
    event_surface_rectangle: &sdl::SDL_FRect,
    long_events: &'ev calendar::EventData,
    week_start: &calendar::Date,
    cell_width: f32,
    top_panel_height: f32,
) -> calendar::render::Rectangles {
    let arguments = calendar::render::Arguments {
        column_width: cell_width,
        column_height: top_panel_height,
        offset_x: event_surface_rectangle.x,
        offset_y: event_surface_rectangle.y,
    };

    let pinned_rectangles_res =
        calendar::render::long_event_rectangles(long_events, week_start, &arguments);

    pinned_rectangles_res.collect()
}

struct WeekData {
    agenda: calendar::obtain::WeekScheduleWithLanes,
    week: Week,
}

impl WeekData {
    const DAYS: u8 = 7;

    fn try_new(
        week_start: &calendar::Date,
        ui_text_factory: &SdlTextCreate,
    ) -> Result<Self, Error> {
        let week: Week = {
            let stream = calendar::DateStream::new(week_start.clone()).take(Self::DAYS as _);
            let week: calendar::ui::Week<Result<sdlext::Text, _>> =
                calendar::ui::create_texts(ui_text_factory, stream);
            validate_week(week)?
        };

        let agenda: calendar::obtain::WeekScheduleWithLanes =
            obtain_agenda(week_start).map_err(Error::DataIsNotAvailable)?;
        Ok(Self { agenda, week })
    }
}

fn unsafe_main() {
    unsafe {
        let ret: Result<(), Error> = sdl_init(
            move |root_window: *mut sdl::SDL_Window, renderer: *mut sdl::SDL_Renderer| {
                let mut text_registry = TextRegistry::new(renderer);
                let mut window_size = sdl::SDL_Point { x: 800, y: 600 };
                _ = sdl::SDL_GetWindowSize(root_window, &mut window_size.x, &mut window_size.y);

                sdl_ttf_init(
                    renderer,
                    move |engine: *mut sdl_ttf::TTF_TextEngine| -> Result<_, Error> {
                        let event_render = RectangleRender { renderer };
                        let fonts = Fonts::new(config::FONT_PATH, config::FONT_PATH)?;
                        let ui_text_factory = SdlTextCreate {
                            engine,
                            font: &fonts.ui,
                        };

                        let week_start: calendar::Date =
                            get_current_week_start().map_err(sdlext::Error::from)?;
                        let week_data = WeekData::try_new(&week_start, &ui_text_factory)?;

                        let mut short_event_rectangles_opt: Option<calendar::render::Rectangles> =
                            None;
                        let mut pinned_rectangles_opt: Option<calendar::render::Rectangles> = None;

                        let title_font_height =
                            sdl_ttf::TTF_GetFontHeight(fonts.title.borrow_mut().ptr());
                        let long_lane_max_count: f32 =
                            week_data.agenda.long.calculate_biggest_clash() as f32;

                        let mut event: sdl::SDL_Event = std::mem::zeroed();
                        'outer_loop: loop {
                            // stage: event handle
                            while sdl::SDL_PollEvent(&mut event as _) {
                                if event.type_ == sdl::SDL_EVENT_QUIT {
                                    break 'outer_loop;
                                }

                                if event.type_ == sdl::SDL_EVENT_WINDOW_RESIZED {
                                    pinned_rectangles_opt.take();
                                    short_event_rectangles_opt.take();
                                    _ = sdl::SDL_GetWindowSize(
                                        root_window,
                                        &mut window_size.x,
                                        &mut window_size.y,
                                    );
                                }
                            }

                            let s =  sdl::SDL_FPoint {
                                x: window_size.x as f32,
                                y: window_size.y as f32,
                            };

                            let view: View = View::new(
                                s,
                                title_font_height,
                                long_lane_max_count,
                                week_data.agenda.long.event_ranges.len(),
                            );

                            let long_event_rectangles: &calendar::render::Rectangles = {
                                let ret: Result<&calendar::render::Rectangles, CalendarError> =
                                    match pinned_rectangles_opt {
                                        Some(ref x) => Ok(x),
                                        None => {
                                            let replacement = create_long_event_rectangles(
                                                &view.event_surface,
                                                &week_data.agenda.long,
                                                &week_start,
                                                view.cell_width,
                                                view.top_panel_height,
                                            );
                                            // TODO: implement a facility which creates the titles
                                            // of the events at once for the "All day" events and
                                            // regular events.  This would allow to prevent
                                            // accidential calling of `clear` twice.
                                            text_registry.clear();
                                            register_event_titles(
                                                &mut text_registry,
                                                &fonts.title,
                                                &week_data.agenda.long.titles,
                                                &replacement,
                                            )?;
                                            Ok(pinned_rectangles_opt.get_or_insert(replacement))
                                        }
                                    };

                                ret?
                            };

                            if short_event_rectangles_opt.is_none() {
                                let new_rectangles = create_short_event_rectangles(
                                    &view.grid_rectangle,
                                    &week_data.agenda.short,
                                    &week_start,
                                );
                                register_event_titles(
                                    &mut text_registry,
                                    &fonts.title,
                                    &week_data.agenda.short.titles,
                                    &new_rectangles,
                                )?;

                                short_event_rectangles_opt.replace(new_rectangles);
                            }

                            let short_event_rectangles =
                                short_event_rectangles_opt.as_ref().unwrap();

                            // stage: render
                            set_color(renderer, Color::from_rgb(config::COLOR_BACKGROUND))?;
                            if !sdl::SDL_RenderClear(renderer) {
                                return Err(sdlext::Error::RenderClearFailed)?;
                            }

                            calendar::render::render_rectangles(
                                long_event_rectangles.iter(),
                                &event_render,
                            )?;

                            calendar::render::render_rectangles(
                                short_event_rectangles.iter(),
                                &event_render,
                            )?;

                            render_grid(renderer, &view.grid_rectangle)?;
                            text_registry.render()?;
                            set_color(renderer, Color::from_rgb(0x111111))?;
                            // render the day names and the dates, render hours
                            let render_week_captions_args =
                                calendar::render::RenderWeekCaptionsArgs::create_for_week(
                                    view.cell_width,
                                    view.cell_height,
                                    view.grid_rectangle.y + 5.,
                                    view.event_surface.x,
                                );

                            week_data
                                .week
                                .render(&SdlTextRender, &render_week_captions_args)
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

    fn render_rectangles<'r, I>(&self, rectangles: I) -> Self::Result
    where
        I: Iterator<Item = &'r calendar::render::Rectangle>,
    {
        unsafe {
            for rect in rectangles {
                set_color(self.renderer, Color::from(rect.color))?;
                let sdl_rect = create_sdl_frect(rect);
                if !sdl::SDL_RenderFillRect(self.renderer, &sdl_rect as _) {
                    return Err(sdlext::Error::RectangleIsNotDrawn);
                }

                let border = sdl::SDL_FRect {
                    x: sdl_rect.x,
                    y: sdl_rect.y,
                    w: sdl_rect.w,
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

fn create_sdl_frect(from: &calendar::render::Rectangle) -> sdl::SDL_FRect {
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
