use calendar::ui::View;
use sdl3_sys as sdl;

use crate::{RectangleRender, SdlTextRender, WeekData};

use super::config;
use sdlext::{Color, set_color};

pub struct RenderData<'a, 'b> {
    pub view: View,
    pub long_event_rectangles: &'a calendar::render::Rectangles,
    pub short_event_rectangles: &'a calendar::render::Rectangles,
    pub week_data: &'a WeekData,
    pub text_registry: &'a crate::TextRegistry<'b>,
}

pub fn render(renderer: &sdlext::Renderer, data: &RenderData) -> sdlext::Result<()> {
    unsafe {
        set_color(renderer, Color::from_rgb(config::COLOR_BACKGROUND))?;
        if !sdl::SDL_RenderClear(renderer.ptr()) {
            return Err(sdlext::Error::RenderClearFailed);
        }

        {
            let event_render = RectangleRender { renderer };
            calendar::render::render_rectangles(data.long_event_rectangles.iter(), &event_render)?;
            calendar::render::render_rectangles(data.short_event_rectangles.iter(), &event_render)?;
        }

        render_grid(renderer, &data.view.grid_rectangle)?;
        data.text_registry.render()?;
        set_color(renderer, Color::from_rgb(0x111111))?;
        // render the day names and the dates, render hours
        let render_week_captions_args = calendar::render::RenderWeekCaptionsArgs::create_for_week(
            data.view.cell_width,
            data.view.cell_height,
            data.view.grid_rectangle.y + 5.,
            data.view.event_surface.x,
        );

        data.week_data
            .week
            .render(&SdlTextRender, &render_week_captions_args)
            .collect::<Result<(), sdlext::TtfError>>()
            .map_err(sdlext::Error::from)?;

        if !sdl::SDL_RenderPresent(renderer.ptr()) {
            return Err(sdlext::Error::RenderIsNotPresent);
        }
    }

    Ok(())
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
