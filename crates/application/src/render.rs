use calendar::ui::View;
use sdl3_sys as sdl;

use crate::DumbFrontend;
use crate::RectangleRender;

use super::config;
use sdlext::Color;

pub struct RenderData<'rect, 'ttc, TTC, F> {
    pub event_viewport: sdl::SDL_Rect,
    pub view: View,
    pub long_event_rectangles: &'rect calendar::render::Rectangles,
    pub short_event_rectangles: &'rect calendar::render::Rectangles,
    pub hours_viewport: sdl::SDL_Rect,
    pub long_event_text_registry: &'ttc TTC,
    pub short_event_text_registry: &'ttc TTC,
    pub frontend: &'ttc F,
    pub dates_viewport: sdl::SDL_Rect,
}

type RD<'a, 'b, 'c, 'd, 'rect, 'ttc> =
    RenderData<'rect, 'ttc, crate::TextTextureRegistry<'d>, DumbFrontend<'a, 'b, 'c>>;

pub fn render(renderer: &sdlext::Renderer, data: &RD) -> sdlext::Result<()> {
    renderer.set_render_draw_color(Color::from_rgb(config::COLOR_BACKGROUND))?;
    renderer.clear()?;
    render_events(renderer, data)?;
    set_render_viewport_context(renderer, &data.hours_viewport, || {
        data.frontend.hour_text_texture_regirsty.render()
    })?;

    set_render_viewport_context(renderer, &data.dates_viewport, || {
        data.frontend.dates_text_texture_regirsty.render()?;
        data.frontend.days_text_texture_regirsty.render()
    })?;
    renderer.present()
}

fn render_events(renderer: &sdlext::Renderer, data: &RD) -> sdlext::Result<()> {
    let event_viewport = data.event_viewport;
    set_render_viewport_context(renderer, &event_viewport, || {
        render_grid(renderer, &data.view.grid_rectangle)?;
        let event_render = RectangleRender { renderer };
        calendar::render::render_rectangles(data.short_event_rectangles.iter(), &event_render)?;
        data.short_event_text_registry.render()?;
        calendar::render::render_rectangles(data.long_event_rectangles.iter(), &event_render)?;
        data.long_event_text_registry.render()
    })
}

fn render_grid(
    renderer: &sdlext::Renderer,
    grid_rectangle: &sdl::SDL_FRect,
) -> Result<(), sdlext::Error> {
    renderer.set_render_draw_color(Color::from_rgb(0x333333))?;
    let row_ratio: f32 = grid_rectangle.h / 24.0;
    for i in 0..24 {
        let ordinate = i as f32 * row_ratio + grid_rectangle.y;
        renderer.render_line(
            grid_rectangle.x,
            ordinate,
            grid_rectangle.w + grid_rectangle.x,
            ordinate,
        )?;
    }

    let col_ratio: f32 = grid_rectangle.w / 7.;
    for i in 0..7 {
        let absciss: f32 = i as f32 * col_ratio + grid_rectangle.x;
        renderer.render_line(
            absciss,
            grid_rectangle.y,
            absciss,
            grid_rectangle.h + grid_rectangle.y,
        )?;
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
