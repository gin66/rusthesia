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

struct RefPosition {
    pos_us: u64,
    at_instant: Option<Instant>,
    scaling_1024: u64
}

struct MidiSequencerThread {
    out_port: usize,
    control: mpsc::Receiver<MidiSequencerCommand>,
    events: Vec<RawMidiTuple>,
    ref_pos: Arc<Mutex<RefPosition>>,
}
impl MidiSequencerThread {
    fn new(
        control: mpsc::Receiver<MidiSequencerCommand>,
        out_port: usize,
        events: Vec<RawMidiTuple>,
        ref_pos: Arc<Mutex<RefPosition>>,
    ) -> MidiSequencerThread {
        MidiSequencerThread { out_port, control, events, ref_pos }
    }
    fn run(&mut self) {
        trace!("Opening connection");
        let midi_out = midir::MidiOutput::new("My Test Output").unwrap();
        let mut conn_out = midi_out.connect(self.out_port, "midir-test").unwrap();
        trace!("Connection opened");
        let mut idx = 0;
        let mut state = SequencerState::Stopped;
        'main: loop {
            state = match state {
                SequencerState::Stopped => {
                    match self.control.recv() {
                        Err(mpsc::RecvError) => break,
                        Ok(MidiSequencerCommand::Ping) => SequencerState::Stopped,
                        Ok(MidiSequencerCommand::Play(pos_us, new_scaling, opt_events)) => 
                            SequencerState::StartPlaying(pos_us, new_scaling, opt_events),
                        Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                            self.ref_pos.lock().unwrap().scaling_1024 = new_scaling as u64;
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
                            self.ref_pos.lock().unwrap().scaling_1024 = new_scaling as u64;
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
                    {
                        let mut ref_pos = self.ref_pos.lock().unwrap();
                        ref_pos.pos_us = pos_us;
                        ref_pos.at_instant = Some(Instant::now());
                        ref_pos.scaling_1024 = new_scaling as u64;
                    }
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
                    let pos_us = self.ref_pos.lock().unwrap().pos_us;
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

                    let scaling_1024 = self.ref_pos.lock().unwrap().scaling_1024;
                    let dt_us = self.events[idx].0 - pos_us;
                    let dt_ms = (dt_us * scaling_1024/1_024_000).min(100).max(1);
                    trace!("sleep {} ms",dt_ms);
                    sleep(Duration::from_millis(dt_ms));

                    {
                        let mut ref_pos = self.ref_pos.lock().unwrap();
                        let elapsed_us = ref_pos.at_instant.unwrap().elapsed().subsec_micros();
                        ref_pos.at_instant = Some(Instant::now());
                        ref_pos.pos_us += elapsed_us as u64 * 1024 / ref_pos.scaling_1024;
                    }
                    SequencerState::Playing
                }
            }
        }
        conn_out.close();
        trace!("Connection closed");
    }
}

pub struct MidiSequencer {
    ref_pos: Arc<Mutex<RefPosition>>,
    control: mpsc::Sender<MidiSequencerCommand>,
}

impl MidiSequencer {
    pub fn new(out_port: usize, events: Vec<RawMidiTuple>) -> MidiSequencer {
        let (tx, rx) = mpsc::channel();
        let ref_pos = Arc::new(Mutex::new(RefPosition {
            pos_us: 0,
            at_instant: None,
            scaling_1024: 1024
        }));
        let ref_pos2 = ref_pos.clone();
        thread::spawn(move || MidiSequencerThread::new(rx, out_port, events, ref_pos2).run());
        MidiSequencer { control: tx, ref_pos }
    }
    pub fn set_pos_us(&self, pos_us: u64) {
        let mut ref_pos = self.ref_pos.lock().unwrap();
        ref_pos.pos_us = pos_us;
        ref_pos.at_instant = None;
    }
    pub fn pos_us(&self) -> u64 {
        let ref_pos = self.ref_pos.lock().unwrap();
        match ref_pos.at_instant.as_ref() {
            None => ref_pos.pos_us,
            Some(instant) => ref_pos.pos_us + instant.elapsed().subsec_micros() as u64 * 1024 / ref_pos.scaling_1024
        }
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
