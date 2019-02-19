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
    time::{Duration, Instant},
};


struct TrackingSource<R>
where
    R: Read + Seek
{
    inner: Decoder<R>,
    samples_read: u32,
    time_base: Instant,
}

impl<R> TrackingSource<R>
where 
    R: Read + Seek
{
    fn new(inner: Decoder<R>) -> TrackingSource<R> {
        TrackingSource {
            inner: inner,
            samples_read: 0,
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
        self.samples_read += 1;
        if self.samples_read % self.inner.sample_rate() == 0 {
            let millis = (self.samples_read as u64 * 1000) / (self.inner.sample_rate() as u64 * self.inner.channels() as u64);
            println!("{:?} {:?}", millis, self.time_base.elapsed());
        }
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
    let mut sink = Sink::new(&device);


    let file = File::open("top.ogg").expect("Couldn't open file");
    let source = TrackingSource::new(Decoder::new(BufReader::new(file)).expect("Couldn't decode file"));
    let silence_source = Zero::<i16>::new(source.channels(), source.sample_rate()).take_duration(Duration::new(0, 10));

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
