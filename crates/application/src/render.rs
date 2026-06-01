use calendar::ui::View;
use sdl3_sys as sdl;

use crate::DumbFrontend;
use crate::RectangleRender;

use super::config;
use sdlext::Color;

pub enum RenderData<'rect, 'frontend, TTC, F> {
    WeekView(WeekViewRenderData<'rect, 'frontend, F>),
    EventView(EventViewRenderData<'frontend, 'rect, TTC>),
}

pub struct EventViewRenderData<'ttc, 'rect, TextObjectRegistry> {
    pub text_registry: &'ttc TextObjectRegistry,
    pub textbox: Option<&'rect sdl::SDL_FRect>,
    pub cursor: Option<&'rect sdl::SDL_FRect>,
}

type ERD<'renderer, 'rect, 'ttc, 'font> =
    EventViewRenderData<'ttc, 'rect, crate::TextObjectRegistry<'font>>;

type RDA<'renderer, 'rect, 'ttc, 'font> =
    RenderData<'rect, 'ttc, crate::TextObjectRegistry<'font>, DumbFrontend<'renderer, 'font>>;

pub struct WeekViewRenderData<'rect, 'frontend, F> {
    pub event_viewport: sdl::SDL_Rect,
    pub view: View,
    pub long_event_rectangles: &'rect calendar::render::Rectangles,
    pub short_event_rectangles: &'rect calendar::render::Rectangles,
    pub hours_viewport: sdl::SDL_Rect,
    pub frontend: &'frontend F,
    pub dates_viewport: sdl::SDL_Rect,
}

type RD<'renderer, 'rect, 'ttc, 'font> =
    WeekViewRenderData<'rect, 'ttc, DumbFrontend<'renderer, 'font>>;

pub fn render(renderer: &sdlext::Renderer, data: &RDA) -> sdlext::Result<()> {
    match data {
        RenderData::WeekView(week_view_render_data) => {
            render_week_view(renderer, week_view_render_data)
        }
        RenderData::EventView(v) => render_event_view(renderer, v),
    }
}

fn render_event_view(renderer: &sdlext::Renderer, data: &ERD) -> sdlext::Result<()> {
    renderer.set_render_draw_color(Color::from_rgb(config::COLOR_BACKGROUND))?;
    renderer.clear()?;
    data.text_registry.render()?;
    if let Some(rect) = data.textbox {
        renderer.set_render_draw_color(Color::WHITE)?;
        renderer.render_rect(rect)?;
    }

    if let Some(rect) = data.cursor {
        renderer.set_render_draw_color(Color::WHITE)?;
        renderer.render_rect(rect)?;
    }
    renderer.present()
}

fn render_week_view(renderer: &sdlext::Renderer, data: &RD) -> sdlext::Result<()> {
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
    let event_render = RectangleRender { renderer };
    calendar::render::render_rectangles(data.long_event_rectangles.iter(), &event_render)?;
    data.frontend.long_event_text_registry.render()?;

    let event_viewport = data.event_viewport;
    set_render_viewport_context(renderer, &event_viewport, || {
        render_short_events(renderer, &data.view.short_event_surface)?;
        let event_render = RectangleRender { renderer };
        calendar::render::render_rectangles(data.short_event_rectangles.iter(), &event_render)?;
        data.frontend.short_event_text_registry.render()
    })
}

fn render_short_events(
    renderer: &sdlext::Renderer,
    short_event_surface: &sdl::SDL_FRect,
) -> Result<(), sdlext::Error> {
    renderer.set_render_draw_color(Color::from_rgb(0x333333))?;
    let row_ratio: f32 = short_event_surface.h / 24.0;
    for i in 0..24 {
        let ordinate = i as f32 * row_ratio + short_event_surface.y;
        renderer.render_line(
            short_event_surface.x,
            ordinate,
            short_event_surface.w + short_event_surface.x,
            ordinate,
        )?;
    }

    let col_ratio: f32 = short_event_surface.w / 7.;
    for i in 0..7 {
        let absciss: f32 = i as f32 * col_ratio + short_event_surface.x;
        renderer.render_line(
            absciss,
            short_event_surface.y,
            absciss,
            short_event_surface.h + short_event_surface.y,
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
