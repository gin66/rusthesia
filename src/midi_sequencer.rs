use log::*;
use std::sync::mpsc;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};

use midly;

#[derive(Debug)]
pub enum MidiEvent {
    NoteOn(u8, u8, u8),
    NoteOff(u8, u8, u8),
    Aftertouch(u8, u8, u8),
    Controller(u8, u8, u8),
    ChannelAftertouch(u8, u8),
    PitchBend(u8, u16),
    ProgramChange(u8, u8),
}
impl MidiEvent {
    pub fn as_raw(&self, _trk_idx: usize) -> Vec<u8> {
        match self {
            MidiEvent::NoteOn(channel, key, pressure) => 
                                vec![
                                    0x90 + channel,
                                    *key,
                                    *pressure,
                                ],
            MidiEvent::NoteOff(channel, key, pressure) => 
                                vec![
                                    0x80 + channel,
                                    *key,
                                    *pressure,
                                ],
            MidiEvent::Controller(channel, control, value) => 
                                vec![
                                    0xb0 + channel,
                                    *control,
                                    *value,
                                ],
            MidiEvent::Aftertouch(channel, key, pressure) => 
                                vec![
                                    0xa0 + channel,
                                    *key,
                                    *pressure,
                                ],
            MidiEvent::ChannelAftertouch(channel, pressure) => 
                                vec![
                                    0xd0 + channel,
                                    *pressure,
                                ],
            MidiEvent::PitchBend(channel, change) => 
                                vec![
                                    0xe0 + channel,
                                    (*change & 0x7f) as u8,
                                    (*change >> 7) as u8,
                                ],
            MidiEvent::ProgramChange(channel, program) => 
                                vec![
                                    0xc0 + channel,
                                    *program,
                                ],
        }
    }
}

pub type RawMidiTuple = (u64, usize, MidiEvent);

enum MidiSequencerCommand {
    Ping,
    Play(u64, u16, Option<Vec<RawMidiTuple>>),
    Scale(u16),
    Stop,
}

enum SequencerState {
    Stopped,
    Playing,
    StartPlaying(u64, u16, Option<Vec<RawMidiTuple>>),
}

struct MidiSequencerThread {
    out_port: usize,
    control: mpsc::Receiver<MidiSequencerCommand>,
    events: Vec<RawMidiTuple>,
    pos_us: Arc<Mutex<u64>>,
}
impl MidiSequencerThread {
    fn new(
        control: mpsc::Receiver<MidiSequencerCommand>,
        out_port: usize,
        events: Vec<RawMidiTuple>,
        pos_us: Arc<Mutex<u64>>,
    ) -> MidiSequencerThread {
        MidiSequencerThread { out_port, control, events, pos_us }
    }
    fn run(&mut self) {
        trace!("Opening connection");
        let midi_out = midir::MidiOutput::new("My Test Output").unwrap();
        let mut conn_out = midi_out.connect(self.out_port, "midir-test").unwrap();
        trace!("Connection opened");
        let mut idx = 0;
        let mut state = SequencerState::Stopped;
        let mut scaling_1024: u64 = 1024;
        let mut ref_time = Instant::now();
        'main: loop {
            state = match state {
                SequencerState::Stopped => {
                    match self.control.recv() {
                        Err(mpsc::RecvError) => break,
                        Ok(MidiSequencerCommand::Ping) => SequencerState::Stopped,
                        Ok(MidiSequencerCommand::Play(pos_us, new_scaling, opt_events)) => 
                            SequencerState::StartPlaying(pos_us, new_scaling, opt_events),
                        Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                            scaling_1024 = new_scaling as u64;
                            SequencerState::Stopped
                        },
                        Ok(MidiSequencerCommand::Stop) => SequencerState::Stopped,
                    }
                },
                SequencerState::Playing => {
                    match self.control.try_recv() {
                        Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => SequencerState::Playing,
                        Ok(MidiSequencerCommand::Ping) => SequencerState::Playing,
                        Ok(MidiSequencerCommand::Play(pos_us, new_scaling, opt_events)) => 
                            SequencerState::StartPlaying(pos_us, new_scaling, opt_events),
                        Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                            scaling_1024 = new_scaling as u64;
                            SequencerState::Playing
                        },
                        Ok(MidiSequencerCommand::Stop) => {
                            for channel in 0..15 {
                                let msg = [0x0b+channel, 123, 0]; // All Notes Off
                                conn_out.send(&msg).unwrap();
                            }
                            SequencerState::Stopped
                        },
                    }
                },
                SequencerState::StartPlaying(_, _, _) => panic!("StartPlaying should not be reachable here")
            };

            state = match state {
                SequencerState::Stopped => SequencerState::Stopped,
                SequencerState::StartPlaying(pos_us, new_scaling, opt_events) => {
                    idx = 0;
                    *self.pos_us.lock().unwrap() = pos_us;
                    ref_time = Instant::now();
                    scaling_1024 = new_scaling as u64;
                    if let Some(events) = opt_events {
                        self.events = events;
                    }
                    while pos_us >= self.events[idx].0 {
                        idx += 1;
                        if idx >= self.events.len() {
                            break 'main;
                        }
                    }
                    SequencerState::Playing
                },
                SequencerState::Playing => {
                    let pos_us = *self.pos_us.lock().unwrap();
                    while pos_us >= self.events[idx].0 {
                        let msg = self.events[idx].2.as_raw(self.events[idx].1);
                        if msg.len() > 0 {
                            conn_out.send(&msg).unwrap();
                        }
                        idx += 1;
                        if idx >= self.events.len() {
                            break 'main;
                        }
                    }

                    let dt_us = (self.events[idx].0 - pos_us).min(50_000).max(1_000);
                    let dt_ms = dt_us * scaling_1024/1_024_000;
                    trace!("sleep {} ms",dt_ms);
                    sleep(Duration::from_millis(dt_ms));

                    let elapsed_us = ref_time.elapsed().subsec_micros();
                    ref_time = Instant::now();
                    *self.pos_us.lock().unwrap() += elapsed_us as u64 * scaling_1024 / 1024;
                    SequencerState::Playing
                }
            }
        }
        conn_out.close();
        trace!("Connection closed");
    }
}

pub struct MidiSequencer {
    pos_us: Arc<Mutex<u64>>,
    control: mpsc::Sender<MidiSequencerCommand>,
}

impl MidiSequencer {
    pub fn new(out_port: usize, events: Vec<RawMidiTuple>) -> MidiSequencer {
        let (tx, rx) = mpsc::channel();
        let pos_us = Arc::new(Mutex::new(0));
        let pos_us2 = pos_us.clone();
        thread::spawn(move || MidiSequencerThread::new(rx, out_port, events, pos_us2).run());
        MidiSequencer { control: tx, pos_us }
    }
    pub fn set_pos_us(&self, pos_us: u64) {
        *self.pos_us.lock().unwrap() = pos_us;
    }
    pub fn pos_us(&self) -> u64 {
        *self.pos_us.lock().unwrap()
    }
    pub fn is_finished(&self) -> bool {
        self.control.send(MidiSequencerCommand::Ping).is_err()
    }
    pub fn play(&self, pos_us: u64, new_scale: u16, opt_events: Option<Vec<RawMidiTuple>>) {
        self.control.send(MidiSequencerCommand::Play(pos_us,new_scale,opt_events)).ok();
    }
    pub fn set_scaling_1024(&self, new_scale: u16) {
        self.control.send(MidiSequencerCommand::Scale(new_scale)).ok();
    }
    pub fn stop(&self) {
        self.control.send(MidiSequencerCommand::Stop).ok();
    }
}
