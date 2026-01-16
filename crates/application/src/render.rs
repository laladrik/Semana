use calendar::ui::View;
use sdl3_sys as sdl;

use crate::{RectangleRender, SdlTextRender, WeekData};

use super::config;
use sdlext::{Color, set_color};

pub struct RenderData<'a, 'b> {
    pub event_viewport: sdl::SDL_Rect,
    pub event_offset: sdl::SDL_FPoint,
    pub view: View,
    pub long_event_rectangles: &'a calendar::render::Rectangles,
    pub short_event_rectangles: &'a calendar::render::Rectangles,
    pub week_data: &'a WeekData,
    pub long_event_text_registry: &'a crate::TextRegistry<'b>,
    pub short_event_text_registry: &'a crate::TextRegistry<'b>,
    pub window_size: sdl3_sys::SDL_Point,
}

pub fn render(renderer: &sdlext::Renderer, data: &RenderData) -> sdlext::Result<()> {
    unsafe {
        set_color(renderer, Color::from_rgb(config::COLOR_BACKGROUND))?;
        if !sdl::SDL_RenderClear(renderer.ptr()) {
            return Err(sdlext::Error::RenderClearFailed);
        }

        let (x, _y) = (data.event_offset.x as i32, data.event_offset.y as i32);
        render_events(renderer, data)?;
        render_hours(renderer, x, data)?;
        render_days(renderer, x, data)?;
        if !sdl::SDL_RenderPresent(renderer.ptr()) {
            return Err(sdlext::Error::RenderIsNotPresent);
        }
    }

    Ok(())
}

fn render_events(renderer: &sdlext::Renderer, data: &RenderData) -> sdlext::Result<()> {
    let event_viewport = data.event_viewport;
    set_render_viewport_context(renderer, &event_viewport, || {
        render_grid(renderer, &data.view.grid_rectangle)?;
        let event_render = RectangleRender { renderer };
        calendar::render::render_rectangles(data.short_event_rectangles.iter(), &event_render)?;
        data.short_event_text_registry.render()?;
        calendar::render::render_rectangles(data.long_event_rectangles.iter(), &event_render)?;
        data.long_event_text_registry.render()?;
        Ok(())
    })
}

fn render_hours(renderer: &sdlext::Renderer, width: i32, data: &RenderData) -> sdlext::Result<()> {
    let hours_viewport = sdl::SDL_Rect {
        x: 10,
        y: data.event_viewport.y + data.view.calculate_top_panel_height() as i32,
        w: width,
        h: data.window_size.y,
    };

    set_render_viewport_context(renderer, &hours_viewport, || {
        let arguments = calendar::render::RenderHoursArgs {
            row_height: data.view.cell_height,
            offset_x: 0.,
            offset_y: data.view.grid_rectangle.y - data.view.calculate_top_panel_height(),
        };

        calendar::render::render_hours(&SdlTextRender, data.week_data.week.hours.iter(), &arguments)
            .collect::<Result<(), sdlext::TtfError>>()
            .map_err(sdlext::Error::from)
    })
}

fn render_days(
    renderer: &sdlext::Renderer,
    horizontal_offset: i32,
    data: &RenderData,
) -> sdlext::Result<()> {
    let dates_viewport = sdl::SDL_Rect {
        x: horizontal_offset,
        y: 0,
        w: data.window_size.x - horizontal_offset,
        h: 200,
    };

    set_render_viewport_context(renderer, &dates_viewport, || {
        let get_arguments = |offset| calendar::render::Arguments {
            column_width: data.view.cell_width,
            column_height: data.view.cell_height,
            offset_x: 0.,
            offset_y: 0. + offset,
        };

        calendar::render::render_weekdays(
            &SdlTextRender,
            data.week_data.week.dates.iter(),
            &get_arguments(10f32),
        )
        .collect::<Result<(), sdlext::TtfError>>()
        .map_err(sdlext::Error::from)?;

        calendar::render::render_weekdays(
            &SdlTextRender,
            data.week_data.week.days.iter(),
            &get_arguments(35f32),
        )
        .collect::<Result<(), sdlext::TtfError>>()
        .map_err(sdlext::Error::from)
    })
}

fn render_grid(
    renderer: &sdlext::Renderer,
    grid_rectangle: &sdl::SDL_FRect,
) -> Result<(), sdlext::Error> {
    unsafe {
        set_color(renderer, Color::from_rgb(0x333333))?;
        let row_ratio: f32 = grid_rectangle.h / 24.0;
        for i in 0..24 {
            let ordinate = i as f32 * row_ratio + grid_rectangle.y;
            let _ = sdl::SDL_RenderLine(
                renderer.ptr(),
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
                renderer.ptr(),
                absciss,
                grid_rectangle.y,
                absciss,
                grid_rectangle.h + grid_rectangle.y,
            );
        }
    }
    Ok(())
}

pub fn set_render_viewport_context<'a, F>(
    renderer: &sdlext::Renderer,
    rect: impl Into<Option<&'a sdl::SDL_Rect>>,
    callback: F,
) -> sdlext::Result<()>
where
    F: Fn() -> sdlext::Result<()>,
{
    sdlext::set_render_viewport(renderer, rect)?;
    let r = callback();
    sdlext::set_render_viewport(renderer, None)?;
    r
}
