extern crate sdl2;

use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
};

use std::{
    time::{Duration, Instant},
};


fn main() {
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    let window = video_subsystem.window("Rhythm Game", 800, 600)
        .position(2500, 300)
        .build()
        .unwrap();

    let mut canvas = window.into_canvas()
        .present_vsync()
        .build()
        .unwrap();
    canvas.set_draw_color(Color::RGB(255, 0, 255));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl.event_pump().unwrap();

    let loop_start = Instant::now();
    let mut frame_count_start = loop_start;
    let mut frame_count = 0;
    let one_second = Duration::new(1, 0);
    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'main;
                },
                Event::KeyDown { keycode: Some(keycode), .. } => {
                    println!("{:?}: {:?}", loop_start.elapsed(), keycode);
                },
                _ => {}
            }
        }

        frame_count += 1;
        if frame_count_start.elapsed() > one_second {
            let count_duration = frame_count_start.elapsed();
            let fps = (frame_count as f64) / 
                ((count_duration.as_secs() as f64) + (count_duration.subsec_millis() as f64) / 1000.0);
            frame_count_start = Instant::now();
            frame_count = 0;
            println!("FPS: {}", fps);
        }

        let bpm = 160;
        let beat_param = ((loop_start.elapsed() * bpm / 60).subsec_millis() as f64) / 1000.0;
        let color = ((1.0 - beat_param).powi(2) * 255.0) as u8;

        canvas.set_draw_color(Color::RGB(color, 0, color));
        canvas.clear();
        canvas.present();
    }

    println!("Hello, world!");
}
