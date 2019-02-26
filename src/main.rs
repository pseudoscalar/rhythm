extern crate rodio;
extern crate sdl2;

use rodio::{
    Decoder,
    Sink,
    source::{Source, Zero},
};

use sdl2::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
};

use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};


struct TrackingSource<R>
where
    R: Read + Seek
{
    inner: Decoder<R>,
    samples_read: Arc<AtomicUsize>,
    time_base: Instant,
}

impl<R> TrackingSource<R>
where 
    R: Read + Seek
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
    R: Read + Seek
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        self.samples_read.fetch_add(1, Ordering::Relaxed);
        self.inner.next()
    }
}

impl<R> Source for TrackingSource<R>
where
    R: Read + Seek
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

fn main() {
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

    let mut canvas = window.into_canvas()
        .present_vsync()
        .build()
        .unwrap();
    canvas.set_draw_color(Color::RGB(255, 0, 255));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl.event_pump().unwrap();

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
                    let samples = sink.samples_written.load(Ordering::Relaxed);
                    let sample_time = samples as f64 / samples_per_sec as f64;

                    let source_samples = samples_read.load(Ordering::Relaxed);
                    let source_sample_time = source_samples as f64 / source_samples_per_sec as f64;

                    let dur = loop_start.elapsed();
                    let clock_time = dur.as_secs() as f64 + (dur.subsec_millis() as f64 / 1000.0);
                    println!("{:?} {:?} {:?}", sample_time, sample_time - source_sample_time, keycode);
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
//            println!("FPS: {}", fps);
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
