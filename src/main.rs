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

struct SdlSystem {
    sdl: Sdl,
    canvas: Canvas<Window>,
    event_pump: EventPump,
}

impl<'a> System<'a> for SdlSystem {
    type SystemData = (Write<'a, InputEvents>,
                       Read<'a, ClearColor>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut input_events, clear_color) = data;

        input_events.0.append(&mut self.event_pump.poll_iter().filter_map(|e| {
            match e {
                Event::KeyDown { keycode, timestamp, .. } => Some(InputEvent { keycode, timestamp }),
                _ => None,
            }
        }).collect::<Vec<InputEvent>>());

        self.canvas.set_draw_color(clear_color.0);
        self.canvas.clear();
        self.canvas.present();
    }
}

struct OmniSystem;

impl<'a> System<'a> for OmniSystem {
    type SystemData = (Write<'a, IsRunning>,
                       Write<'a, InputEvents>,
                       Write<'a, ClearColor>,
                       Option<Read<'a, Device>>,
                       Option<Read<'a, Sink>>);

    fn run(&mut self, data: Self::SystemData) {
        let (mut isRunning, mut input_events, mut clear_color, maybe_device, maybe_sink) = data;

        if let (Some(device), Some(sink)) = (maybe_device, maybe_sink) {
            let format = device.default_output_format().expect("Couldn't get default output format");
            let samples_per_sec = format.channels as u32 * format.sample_rate.0;

            let samples = sink.samples_written.load(Ordering::Relaxed);
            let sample_time = samples as f64 / samples_per_sec as f64;
            let beat_time = sample_time * 160.0 / 60.0;

            let color = ((1.0 - beat_time.fract()).powi(2) * 255.0) as u8;
            clear_color.0 = Color::RGB(color, 0, color);
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
    world.add_resource(sink);
    world.add_resource(device);
    world.add_resource(InputEvents(Vec::new()));

    let sdl_system = SdlSystem { sdl, canvas, event_pump };

    let mut dispatcher = DispatcherBuilder::new()
        .with_thread_local(sdl_system)
        .with(OmniSystem, "omni_system", &[])
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
