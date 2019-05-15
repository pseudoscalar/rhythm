use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color as SdlColor,
    render::Canvas,
    video::Window,
    EventPump,
    Sdl,
};

use specs::prelude::*;

use crate::render::{
    ClearColor,
    Color,
    Position,
    Rectangle,
};

impl Into<SdlColor> for Color {
    fn into(self) -> SdlColor {
        SdlColor::RGB(self.r, self.g, self.b)
    }
}

#[derive(Debug)]
pub struct InputEvent {
    pub timestamp: u32,
    pub keycode: Option<Keycode>,
}

#[derive(Default)]
pub struct InputEvents(pub Vec<InputEvent>);

#[derive(Default)]
pub struct SdlRects(Vec<(SdlColor, sdl2::rect::Rect)>);

pub struct SdlSystem {
    _sdl: Sdl,
    canvas: Canvas<Window>,
    event_pump: EventPump,
}

impl SdlSystem {
    pub fn new(sdl: Sdl, canvas: Canvas<Window>, event_pump: EventPump) -> SdlSystem {
        SdlSystem { _sdl: sdl, canvas, event_pump }
    }
}

impl<'a> System<'a> for SdlSystem {
    type SystemData = (Write<'a, InputEvents>,
                       Read<'a, SdlRects>,
                       Read<'a, ClearColor>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut input_events, sdl_rects, clear_color) = data;

        input_events.0.clear();
        input_events.0.append(&mut self.event_pump.poll_iter().filter_map(|e| {
            match e {
                Event::KeyDown { keycode, timestamp, .. } => Some(InputEvent { keycode, timestamp }),
                _ => None,
            }
        }).collect::<Vec<InputEvent>>());

        self.canvas.set_draw_color(clear_color.0);
        self.canvas.clear();
        for (color, rect) in sdl_rects.0.iter() {
            self.canvas.set_draw_color(color.clone());
            if let Err(e) = self.canvas.fill_rect(Some(rect.clone())) { dbg!(e); }
        }
        self.canvas.present();
    }
}

pub struct RenderingSystem;

impl<'a> System<'a> for RenderingSystem {
    type SystemData = (ReadStorage<'a, Rectangle>,
                       ReadStorage<'a, Position>,
                       ReadStorage<'a, Color>,
                       Write<'a, SdlRects>);

    fn run(&mut self, data: Self::SystemData) {
        let (rect_storage, position_storage, color_storage, mut sdl_rects) = data;

        sdl_rects.0.clear();

        for (rect, pos, color) in (&rect_storage, &position_storage, &color_storage).join() {
            sdl_rects.0.push((
                    color.clone().into(),
                    sdl2::rect::Rect::from_center(
                        (pos.x.round() as i32, pos.y.round() as i32),
                        rect.width.round() as u32,
                        rect.height.round() as u32,
                    )
            ));
        }
    }
}


