use specs::prelude::*;

use crate::{
    AudioTime,
    TargetInput,
    sdl::{InputEvent, InputEvents},
};

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
pub struct TargetBarTime(pub u64);

impl Component for TargetBarTime {
    type Storage = VecStorage<Self>;
}

#[derive(Default)]
pub struct AudioContext {
    pub milli_bpm: u64,
    pub first_beat_offset: u64,
    pub beats_per_bar: u8,
    pub bar_millis: u64,
    pub beat_millis: u64,
}

impl AudioContext {
    pub fn new(milli_bpm: u64, first_beat_offset: u64, beats_per_bar: u8) -> AudioContext {
        let beat_millis = 60_000_000  / milli_bpm;
        let bar_millis = (60_000_000 * beats_per_bar as u64) / milli_bpm;
        AudioContext {
            milli_bpm,
            first_beat_offset,
            beats_per_bar,
            bar_millis,
            beat_millis,
        }
    }

    pub fn make_bar_time(&self, multiple: u64, division: u64, index: u64) -> TargetBarTime {
        TargetBarTime((self.beat_millis * index * multiple) / division)
    }
}

#[derive(Debug)]
#[derive(Default)]
#[derive(Clone)]
#[derive(Copy)]
pub struct RhythmCombo;

impl Component for RhythmCombo {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
pub struct BarIndex(pub u64);

impl Component for BarIndex {
    type Storage = VecStorage<Self>;
}

pub(crate) struct BarIndexTaggingSystem;

impl<'a> System<'a> for BarIndexTaggingSystem {
    type SystemData = (Entities<'a>,
                       Read<'a, AudioTime>,
                       Read<'a, AudioContext>,
                       Read<'a, InputEvents>,
                       ReadStorage<'a, TargetInput>,
                       ReadStorage<'a, TargetBarTime>,
                       WriteStorage<'a, BarIndex>);

    fn run(&mut self, data: Self::SystemData) {
        let (
            entities,
            audio_time,
            audio_context,
            input_events,
            target_input_storage,
            target_bar_time_storage,
            mut bar_index_storage
        ) = data;

        for event in &input_events.0 {
            match *event {
                InputEvent { keycode: Some(keycode), timestamp } => {
                    let targets_hit: Vec<_> = (&*entities, &target_input_storage, &target_bar_time_storage, !&bar_index_storage)
                        .join()
                        .filter(|(_, input, _, _)| input.0 == keycode)
                        .filter_map(|(entity, _, target_bar_time, _)| {

                            let nearest_bar = (audio_time.0.saturating_sub(target_bar_time.0) + audio_context.bar_millis / 2) / audio_context.bar_millis;
                            let target_time = nearest_bar * audio_context.bar_millis + target_bar_time.0;
                            let milli_error = if audio_time.0 > target_time {
                                audio_time.0 - target_time
                            } else {
                                target_time - audio_time.0
                            };

                            dbg!((target_bar_time, nearest_bar, target_time, milli_error));

                            if milli_error < 100 {
                                Some((entity, nearest_bar, milli_error))
                            } else {
                                None
                            }
                        }).collect();

                    for hit in targets_hit {
                        if let Err(err) = bar_index_storage.insert(hit.0, BarIndex(hit.1)) {
                            dbg!(err);
                        }
                    }
                },
                _ => {},
            }
        }
    }
}
