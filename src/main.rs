use rodio::{
    Decoder,
    Device,
    Sink,
    source::{Source, Zero},
};

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
struct DebugFlag(bool);

#[derive(Default)]
struct AudioContext {
    milli_bpm: u64,
    first_beat_offset: u64,
}

#[derive(Default)]
struct SdlRects(Vec<(SdlColor, sdl2::rect::Rect)>);
struct ClearColor(Color);

impl Default for ClearColor {
    fn default() -> ClearColor { ClearColor(Color::rgb(0,0,0)) }
}

#[derive(Debug)]
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
        for (color, rect) in sdl_rects.0.iter() {
            self.canvas.set_draw_color(color.clone());
            self.canvas.fill_rect(Some(rect.clone()));
        }
        self.canvas.present();
    }
}

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
                       ReadStorage<'a, TargetTime>, 
                       WriteStorage<'a, Color>);

    fn run(&mut self, data: Self::SystemData) {
        let (entities, mut isRunning, mut input_events, mut clear_color, mut audio_time, mut debug_flag, audio_context, maybe_device, maybe_sink, target_input_storage, target_time_storage, mut color_storage) = data;

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
                    isRunning.0 = false;
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
                    if let Some((entity, _, target_time)) = (&*entities, &target_input_storage, &target_time_storage).join().filter(|(_, input, time)| input.0 == keycode && time.0 < audio_time.0 + 150).max_by_key(|(_, _, time)| time.0) {
                        let timestamp = timestamp as u64 + 45;
                        let error = if timestamp > target_time.0 {
                            dbg!(timestamp - target_time.0)
                        } else {
                            dbg!(target_time.0 - timestamp)
                        };
                        
                        if let Some(color) = color_storage.get_mut(entity) {
                            *color = if error <= 45 {
                                Color::rgb(255, 255, 0)
                            } else if error <= 90 {
                                Color::rgb(255, 128, 128)
                            } else if error <= 135 {
                                Color::rgb(255, 0, 255)
                            } else {
                                Color::rgb(255, 0, 0)
                            }
                        }

                    }
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

struct TimeScrollingSystem;

impl<'a> System<'a> for TimeScrollingSystem {
    type SystemData = (Read<'a, AudioTime>,
                       ReadStorage<'a, TargetTime>,
                       WriteStorage<'a, Position>);

    fn run(&mut self, data: Self::SystemData) {
        let (audio_time, target_time_storage, mut position_storage) = data;

        for (target_time, pos) in (&target_time_storage, &mut position_storage).join() {
            let time_delta = target_time.0 as i64 - audio_time.0 as i64;
            let displacement = time_delta as f64 * 0.8;

            pos.y = 200.0 + displacement;
        }
    }
}

struct QuaverGeneratorSystem;

impl<'a> System<'a> for QuaverGeneratorSystem {
    type SystemData = (Entities<'a>,
                    Read<'a, AudioTime>,
                    Read<'a, DebugFlag>,
                    Read<'a, AudioContext>,
                    WriteStorage<'a, TargetTime>,
                    WriteStorage<'a, TargetInput>,
                    WriteStorage<'a, Rectangle>,
                    WriteStorage<'a, Color>,
                    WriteStorage<'a, Position>);

    fn run(&mut self, data: Self::SystemData) {
        let (entities, audio_time, debug_flag, audio_context, mut target_time_storage, mut target_input_storage, mut rect_storage, mut color_storage, mut position_storage) = data;

        let max_time = target_time_storage.join().max_by_key(|time| time.0).cloned().unwrap_or(TargetTime(audio_time.0));
        if debug_flag.0 {
            dbg!(max_time);
        }
        let adj_max_time = max_time.0.saturating_sub(audio_context.first_beat_offset);
        let mut max_quaver = (adj_max_time * audio_context.milli_bpm * 2) / 60_000_000;
        let remainder = (adj_max_time * audio_context.milli_bpm * 2) % 60_000_000;
        if remainder > 30_000 {
            max_quaver += 1;
        }
        let quaver_index = max_quaver + 1;
        let next_time = (quaver_index * 60_000_000) / (audio_context.milli_bpm * 2) + audio_context.first_beat_offset;

        if next_time > audio_time.0 && next_time - audio_time.0 < 1000 {
            let next_note = entities.create();
            let (x_offset, input) = if quaver_index % 2 == 0 {
                (-40.0, TargetInput(Keycode::Left))
            }
            else {
                (40.0, TargetInput(Keycode::Right))
            };
            
            target_time_storage.insert(next_note, TargetTime(next_time));
            rect_storage.insert(next_note, Rectangle{ width: 100.0, height: 40.0 });
            position_storage.insert(next_note, Position{ x: 430.0 + x_offset, y: 0.0});
            target_input_storage.insert(next_note, input);
            color_storage.insert(next_note, Color::rgb(0,0,0));
        }
    }
}

struct TimedItemReaper;

impl<'a> System<'a> for TimedItemReaper {
    type SystemData = (Entities<'a>,
                       Read<'a, AudioTime>,
                       ReadStorage<'a, TargetTime>);

    fn run(&mut self, data: Self::SystemData) {
        let (entities, audio_time, target_time_storage) = data;

        for (entity, _) in (&*entities, &target_time_storage).join().filter(|(_, time)| time.0 + 1000 < audio_time.0) {
            entities.delete(entity);
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

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color {r, g, b}
    }
}

impl Component for Color {
    type Storage = VecStorage<Self>;
}

impl Into<SdlColor> for Color {
    fn into(self) -> SdlColor {
        SdlColor::RGB(self.r, self.g, self.b)
    }
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

    let clear_color = Color::rgb(255, 255, 255);
    let mut canvas = window.into_canvas()
        .build()
        .unwrap();
    canvas.set_draw_color(clear_color);
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl.event_pump().unwrap();

    world.add_resource(IsRunning(true));
    world.add_resource(DebugFlag(false));
    world.add_resource(ClearColor(clear_color));
    world.add_resource(AudioTime(0));
    world.add_resource(AudioContext{
        milli_bpm: (160_000 - 150),
        first_beat_offset: 110,
    });
    world.add_resource(sink);
    world.add_resource(device);
    world.add_resource(InputEvents(Vec::new()));
    world.add_resource(SdlRects(Vec::new()));

    world.register::<Position>();
    world.register::<Color>();
    world.register::<Rectangle>();
    world.register::<TargetTime>();
    world.register::<TargetInput>();

    world.create_entity()
        .with(Rectangle { width: 3000.0, height: 1.0 })
        .with(Color::rgb(0,0,0))
        .with(Position { x: 0.0, y: 200.0 })
        .build();
    

    let sdl_system = SdlSystem { sdl, canvas, event_pump };

    let mut dispatcher = DispatcherBuilder::new()
        .with_thread_local(sdl_system)
        .with(OmniSystem, "omni_system", &[])
        .with(RenderingSystem, "rendering_system", &[])
        .with(TimeScrollingSystem, "time_scrolling_system", &[])
        .with(QuaverGeneratorSystem, "quaver_generator_system", &[])
        .with(TimedItemReaper, "timed_item_reaper", &[])
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
