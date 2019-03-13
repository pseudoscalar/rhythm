use rodio::{
    Decoder,
    Device,
    Sink,
    source::{Source, Zero},
};

use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    render::Canvas,
    video::Window,
    EventPump,
    Sdl,
};

use specs::prelude::*;

use std::{
    fs::File,
    io::{BufReader, Seek},
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::{Duration, Instant},
    vec::Vec,
};


struct TrackingSource<R>
where
    R: std::io::Read + Seek
{
    inner: Decoder<R>,
    samples_read: Arc<AtomicUsize>,
    time_base: Instant,
}

impl<R> TrackingSource<R>
where 
    R: std::io::Read + Seek
{
    fn new(inner: Decoder<R>) -> TrackingSource<R> {
        TrackingSource {
            inner: inner,
            samples_read: Arc::new(AtomicUsize::new(0)),
            time_base: Instant::now(),
        }
    }
}

impl<R> Iterator for TrackingSource<R>
where 
    R: std::io::Read + Seek
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        self.samples_read.fetch_add(1, Ordering::Relaxed);
        self.inner.next()
    }
}

impl<R> Source for TrackingSource<R>
where
    R: std::io::Read + Seek
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

#[derive(Default)]
struct SdlRects(Vec<sdl2::rect::Rect>);
struct ClearColor(Color);

impl Default for ClearColor {
    fn default() -> ClearColor { ClearColor(Color::RGB(0,0,0)) }
}

struct InputEvent {
    timestamp: u32,
    keycode: Option<Keycode>,
}

#[derive(Default)]
struct InputEvents(Vec<InputEvent>);

#[derive(Default)]
struct IsRunning(bool);

#[derive(Default)]
struct AudioTime(u64);

struct SdlSystem {
    sdl: Sdl,
    canvas: Canvas<Window>,
    event_pump: EventPump,
}

impl<'a> System<'a> for SdlSystem {
    type SystemData = (Write<'a, InputEvents>,
                       Read<'a, SdlRects>,
                       Read<'a, ClearColor>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut input_events, sdl_rects, clear_color) = data;

        input_events.0.append(&mut self.event_pump.poll_iter().filter_map(|e| {
            match e {
                Event::KeyDown { keycode, timestamp, .. } => Some(InputEvent { keycode, timestamp }),
                _ => None,
            }
        }).collect::<Vec<InputEvent>>());

        self.canvas.set_draw_color(clear_color.0);
        self.canvas.clear();
        self.canvas.set_draw_color(Color::RGB(0,0,0));
        self.canvas.fill_rects(&(sdl_rects.0)[..]);
        self.canvas.present();
    }
}

struct OmniSystem;

impl<'a> System<'a> for OmniSystem {
    type SystemData = (Write<'a, IsRunning>,
                       Write<'a, InputEvents>,
                       Write<'a, ClearColor>,
                       Write<'a, AudioTime>,
                       Option<Read<'a, Device>>,
                       Option<Read<'a, Sink>>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut isRunning, mut input_events, mut clear_color, mut audio_time, maybe_device, maybe_sink) = data;

        if let (Some(device), Some(sink)) = (maybe_device, maybe_sink) {
            let format = device.default_output_format().expect("Couldn't get default output format");
            let samples_per_sec = format.channels as u32 * format.sample_rate.0;

            let samples = sink.samples_written.load(Ordering::Relaxed);
            let sample_time = samples as f64 / samples_per_sec as f64;
            let beat_time = sample_time * 160.0 / 60.0;

            let color = ((1.0 - beat_time.fract()).powi(2) * 255.0) as u8;
            clear_color.0 = Color::RGB(color, 0, color);

            audio_time.0 = (sample_time * 1000.0) as u64;
        }

        for event in input_events.0.drain(..) {
            match event {
                InputEvent { keycode: Some(Keycode::Escape), .. } => {
                    isRunning.0 = false;
                },
                _ => {},
            }
        }
    }
}

struct RenderingSystem;

impl<'a> System<'a> for RenderingSystem {
    type SystemData = (ReadStorage<'a, Rectangle>,
                       ReadStorage<'a, Position>,
                       Write<'a, SdlRects>);

    fn run(&mut self, data: Self::SystemData) {
        let (rect_storage, position_storage, mut sdl_rects) = data;

        sdl_rects.0.clear();

        for (rect, pos) in (&rect_storage, &position_storage).join() {
            sdl_rects.0.push(sdl2::rect::Rect::from_center(
                (pos.x.round() as i32, pos.y.round() as i32),
                rect.width.round() as u32,
                rect.height.round() as u32,
            ));
        }
    }
}

struct TimeScrollingSystem;

impl<'a> System<'a> for TimeScrollingSystem {
    type SystemData = (Read<'a, AudioTime>,
                       ReadStorage<'a, TargetTime>,
                       WriteStorage<'a, Position>);

    fn run(&mut self, data: Self::SystemData) {
        let (audio_time, target_time_storage, mut position_storage) = data;

        for (target_time, pos) in (&target_time_storage, &mut position_storage).join() {
            let time_delta = target_time.0 as i64 - audio_time.0 as i64;
            let displacement = time_delta as f64 * 0.5;

            pos.y = 200.0 + displacement;
        }
    }
}

struct QuaverGeneratorSystem;

impl<'a> System<'a> for QuaverGeneratorSystem {
    type SystemData = (Entities<'a>,
                    Read<'a, AudioTime>,
                    WriteStorage<'a, TargetTime>,
                    WriteStorage<'a, Rectangle>,
                    WriteStorage<'a, Position>);

    fn run(&mut self, data: Self::SystemData) {
        let (entities, audio_time, mut target_time_storage, mut rect_storage, mut position_storage) = data;

        if let Some(max_time) = target_time_storage.join().max_by_key(|time| time.0) {
            let mut max_quaver = (max_time.0 * 160 * 2) / 60_000;
            let remainder = (max_time.0 * 160 * 2) % 60_000;
            if remainder < 100 {
                max_quaver += 1;
            }
            let next_time = ((max_quaver + 1) * 60_000) / (160 * 2);

            if next_time - audio_time.0 < 1000 {
                let next_note = entities.create();
                
                target_time_storage.insert(next_note, TargetTime(next_time));
                rect_storage.insert(next_note, Rectangle{ width: 100.0, height: 40.0 });
                position_storage.insert(next_note, Position{ x: 430.0, y: 0.0});
            }
        } 
        else {
            let first_note = entities.create();
            target_time_storage.insert(first_note, TargetTime(0));
            rect_storage.insert(first_note, Rectangle{ width: 100.0, height: 100.0 });
            position_storage.insert(first_note, Position{ x: 430.0, y: 0.0});
        } 
    }
}

#[derive(Debug)]
struct Position {
    x: f64,
    y: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct Rectangle {
    width: f64,
    height: f64,
}

impl Component for Rectangle {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct TargetTime(u64);

impl Component for TargetTime {
    type Storage = VecStorage<Self>;
}

fn main() {
    let mut world = World::new();


    let device  = rodio::default_output_device().expect("Couldn't get default audio device");
    let format = device.default_output_format().expect("Couldn't get default output format");
    let samples_per_sec = format.channels as u32 * format.sample_rate.0;
    let mut sink = Sink::new(&device);
    let loop_start = Instant::now();


    let file = File::open("top-fixed.ogg").expect("Couldn't open file");
    let source = TrackingSource::new(Decoder::new(BufReader::new(file)).expect("Couldn't decode file"));
    let silence_source = Zero::<i16>::new(source.channels(), source.sample_rate()).take_duration(Duration::new(0, 10));

    let samples_read = source.samples_read.clone();
    let source_samples_per_sec = source.channels() as u32 * source.sample_rate();

    sink.set_volume(0.05);
    sink.append(silence_source);
    sink.append(source);

    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();

    let window = video_subsystem.window("Rhythm Game", 800, 600)
        .position(2500, 300)
        .build()
        .unwrap();

    let clear_color = Color::RGB(255, 0, 255);
    let mut canvas = window.into_canvas()
        .build()
        .unwrap();
    canvas.set_draw_color(clear_color);
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl.event_pump().unwrap();

    world.add_resource(IsRunning(true));
    world.add_resource(ClearColor(clear_color));
    world.add_resource(AudioTime(0));
    world.add_resource(sink);
    world.add_resource(device);
    world.add_resource(InputEvents(Vec::new()));
    world.add_resource(SdlRects(Vec::new()));

    world.register::<Position>();
    world.register::<Rectangle>();
    world.register::<TargetTime>();

    let square = Rectangle { width: 100.1, height: 100.1 };
    world.create_entity().with(square).with(Position { x:30.1, y: 200.1 }).build();
    world.create_entity().with(square).with(Position { x:230.1, y: 200.1 }).build();

    /*world.create_entity()
        .with(square)
        .with(Position { x:430.1, y: 200.1 })
        .with(TargetTime(12_000))
        .build();
    */

    let sdl_system = SdlSystem { sdl, canvas, event_pump };

    let mut dispatcher = DispatcherBuilder::new()
        .with_thread_local(sdl_system)
        .with(OmniSystem, "omni_system", &[])
        .with(RenderingSystem, "rendering_system", &[])
        .with(TimeScrollingSystem, "time_scrolling_system", &[])
        .with(QuaverGeneratorSystem, "quaver_generator_system", &[])
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
