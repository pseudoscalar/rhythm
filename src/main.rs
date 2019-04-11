use rodio::{
    Decoder,
    Device,
    Sink,
};

use sdl2::keyboard::Keycode;

use specs::prelude::*;

use std::{
    fs::File,
    io::BufReader,
    sync::atomic::Ordering,
    vec::Vec,
};

mod render;
use crate::render::{
    Color,
    ClearColor,
    Position,
    Rectangle,
};

//mod rodio_ext;

mod sdl;
use crate::sdl::{
    InputEvent,
    InputEvents,
    RenderingSystem,
    SdlRects,
    SdlSystem,
};

#[derive(Default)]
struct DebugFlag(bool);

#[derive(Default)]
struct AudioContext {
    milli_bpm: u64,
    first_beat_offset: u64,
    beats_per_bar: u8,
}

#[derive(Default)]
struct IsRunning(bool);

#[derive(Default)]
struct AudioTime(u64);

struct OmniSystem;

impl<'a> System<'a> for OmniSystem {
    type SystemData = (Entities<'a>,
                       Write<'a, IsRunning>,
                       Write<'a, InputEvents>,
                       Write<'a, ClearColor>,
                       Write<'a, AudioTime>,
                       Write<'a, DebugFlag>,
                       Read<'a, AudioContext>,
                       Option<Read<'a, Device>>,
                       Option<Read<'a, Sink>>,
                       ReadStorage<'a, TargetInput>,
                       ReadStorage<'a, TargetBarRhythm>, 
                       WriteStorage<'a, Color>);

    fn run(&mut self, data: Self::SystemData) {
        let (entities, mut is_running, mut input_events, mut clear_color, mut audio_time, mut debug_flag, audio_context, maybe_device, maybe_sink, target_input_storage, target_rhythm_storage, mut color_storage) = data;

        if let (Some(device), Some(sink)) = (maybe_device, maybe_sink) {
            let format = device.default_output_format().expect("Couldn't get default output format");
            let samples_per_sec = format.channels as u32 * format.sample_rate.0;

            let samples = sink.samples_written.load(Ordering::Relaxed);
            let sample_time = samples as f64 / samples_per_sec as f64 - audio_context.first_beat_offset as f64 / 1000.0;
            let beat_time = sample_time * (audio_context.milli_bpm / 1000) as f64 / 60.0;

            let color = ((1.0 - beat_time.fract()).powi(2) * 128.0) as u8;
            let color = 255 - color;
            clear_color.0 = Color::rgb(color, color, color);

            audio_time.0 = (sample_time * 1000.0) as u64;
        }

        debug_flag.0 = false;
        for event in input_events.0.drain(..) {
            match event {
                InputEvent { keycode: Some(Keycode::Escape), .. } => {
                    is_running.0 = false;
                },
                InputEvent { keycode: Some(Keycode::Backquote), timestamp } => {
                    debug_flag.0 = true;

                    if audio_time.0 > timestamp as u64 {
                        dbg!(audio_time.0 - timestamp as u64);
                    } else {
                        dbg!(("-", timestamp as u64 - audio_time.0));
                    }
                },
                InputEvent { keycode: Some(keycode), timestamp } => {
                    let targets_hit = (&*entities, &target_input_storage, &target_rhythm_storage)
                        .join()
                        .filter(|(_, input, _)| input.0 == keycode)
                        .filter_map(|(entity, _, target_rhythm)| {
                            let target_bar_time = target_rhythm.0 as u64 * 60_000_000 / ((1<<(MAX_BEAT_DIVISION-1)) * audio_context.milli_bpm);
                            let milli_error = 0;

                            dbg!(target_bar_time);

                            Some((entity, milli_error))
                        });

                    for _ in targets_hit {}
                },
                _ => {},
            }
        }
    }
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct TargetTime(u64);

impl Component for TargetTime {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct TargetBarRhythm(u16);
const MAX_BEAT_DIVISION: u8 = 4;

impl Component for TargetBarRhythm {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct TargetInput(Keycode);

impl Component for TargetInput {
    type Storage = VecStorage<Self>;
}

fn main() {
    let mut world = World::new();


    let device  = rodio::default_output_device().expect("Couldn't get default audio device");
    let sink = Sink::new(&device);

    let file = File::open("top-fixed.ogg").expect("Couldn't open file");
    let source = Decoder::new(BufReader::new(file)).expect("Couldn't decode file");

    sink.set_volume(0.05);
    sink.append(source);

    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    let window = video_subsystem.window("Rhythm Game", 800, 600)
        .position(2500, 300)
        .build()
        .unwrap();

    let clear_color = Color::rgb(255, 255, 255);
    let mut canvas = window.into_canvas()
        .build()
        .unwrap();
    canvas.set_draw_color(clear_color);
    canvas.clear();
    canvas.present();

    let event_pump = sdl.event_pump().unwrap();

    world.add_resource(IsRunning(true));
    world.add_resource(DebugFlag(false));
    world.add_resource(ClearColor(clear_color));
    world.add_resource(AudioTime(0));
    world.add_resource(AudioContext{
        milli_bpm: (160_000 - 150),
        first_beat_offset: 110,
        beats_per_bar: 4,
    });
    world.add_resource(sink);
    world.add_resource(device);
    world.add_resource(InputEvents(Vec::new()));
    world.add_resource(SdlRects::default());

    world.register::<Position>();
    world.register::<Color>();
    world.register::<Rectangle>();
    world.register::<TargetBarRhythm>();
    world.register::<TargetInput>();

    world.create_entity()
        .with(Rectangle { width: 3000.0, height: 1.0 })
        .with(Color::rgb(0,0,0))
        .with(Position { x: 0.0, y: 200.0 })
        .build();

    world.create_entity()
        .with(TargetBarRhythm(0))
        .with(TargetInput(Keycode::Left))
        .build();

    world.create_entity()
        .with(TargetBarRhythm(3*(1<<(MAX_BEAT_DIVISION-1))))
        .with(TargetInput(Keycode::Right))
        .build();

    let sdl_system = SdlSystem::new(sdl, canvas, event_pump);

    let mut dispatcher = DispatcherBuilder::new()
        .with_thread_local(sdl_system)
        .with(OmniSystem, "omni_system", &[])
        .with(RenderingSystem, "rendering_system", &[])
        .build();

    'main: loop {
        dispatcher.dispatch(&mut world.res);
        world.maintain();
        if !world.read_resource::<IsRunning>().0 {
            break 'main;
        }
    }
    println!("Hello, world!");
}
