use log::*;
use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use midly;

use crate::time_controller::{TimeController, TimeListener, TimeListenerTrait};
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
    pub fn as_raw(
        &self,
        trk_idx: usize,
        opt_key_pressed: Option<&mut HashSet<(usize, u8, u8)>>,
    ) -> Vec<u8> {
        match self {
            MidiEvent::NoteOn(channel, key, pressure) => {
                if let Some(key_pressed) = opt_key_pressed {
                    key_pressed.insert((trk_idx, *channel, *key));
                }
                vec![0x90 + channel, *key, *pressure]
            }
            MidiEvent::NoteOff(channel, key, pressure) => {
                if let Some(key_pressed) = opt_key_pressed {
                    key_pressed.remove(&(trk_idx, *channel, *key));
                }
                vec![0x80 + channel, *key, *pressure]
            }
            MidiEvent::Controller(channel, control, value) => {
                vec![0xb0 + channel, *control, *value]
            }
            MidiEvent::Aftertouch(channel, key, pressure) => vec![0xa0 + channel, *key, *pressure],
            MidiEvent::ChannelAftertouch(channel, pressure) => vec![0xd0 + channel, *pressure],
            MidiEvent::PitchBend(channel, change) => {
                vec![0xe0 + channel, (*change & 0x7f) as u8, (*change >> 7) as u8]
            }
            MidiEvent::ProgramChange(channel, program) => vec![0xc0 + channel, *program],
        }
    }
}

pub type RawMidiTuple = (u64, usize, MidiEvent);

enum MidiSequencerCommand {
    Ping,
    SetPosition(i64),
    SetEvents(Vec<RawMidiTuple>),
    Play(i64),
    Scale(u16),
    Stop,
}

enum SequencerState {
    Stopped,
    Playing,
    StartPlaying(i64),
    EOF
}

struct MidiSequencerThread {
    out_port: usize,
    control: mpsc::Receiver<MidiSequencerCommand>,
    events: Vec<RawMidiTuple>,
    time_control: TimeController,
}
impl MidiSequencerThread {
    fn new(
        control: mpsc::Receiver<MidiSequencerCommand>,
        out_port: usize,
        time_control: TimeController,
    ) -> MidiSequencerThread {
        MidiSequencerThread {
            out_port,
            control,
            events: vec![],
            time_control,
        }
    }
    fn run(&mut self) {
        use crate::midi_sequencer::SequencerState::*;
        trace!("Opening connection");
        let midi_out = midir::MidiOutput::new("My Test Output").unwrap();
        let mut conn_out = midi_out.connect(self.out_port, "midir-test").unwrap();
        trace!("Connection opened");
        let mut idx = 0;
        let mut state = EOF;
        let mut key_pressed = HashSet::new();
        loop {
            state = match state {
                EOF => match self.control.recv() {
                    Err(mpsc::RecvError) => break,
                    Ok(MidiSequencerCommand::Stop)
                    | Ok(MidiSequencerCommand::Ping) => EOF,
                    Ok(MidiSequencerCommand::Play(pos_us)) => StartPlaying(pos_us),
                    Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                        self.time_control.set_scaling_1000(new_scaling);
                        EOF
                    }
                    Ok(MidiSequencerCommand::SetPosition(pos_us)) => {
                        self.time_control.set_pos_us(pos_us);
                        EOF
                    }
                    Ok(MidiSequencerCommand::SetEvents(events)) => {
                        self.events = events;
                        Stopped
                    }
                },
                Stopped => match self.control.recv() {
                    Err(mpsc::RecvError) => break,
                    Ok(MidiSequencerCommand::Ping) => Stopped,
                    Ok(MidiSequencerCommand::Play(pos_us)) => StartPlaying(pos_us),
                    Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                        self.time_control.set_scaling_1000(new_scaling);
                        Stopped
                    }
                    Ok(MidiSequencerCommand::SetPosition(pos_us)) => {
                        self.time_control.set_pos_us(pos_us);
                        Stopped
                    }
                    Ok(MidiSequencerCommand::SetEvents(events)) => {
                        self.events = events;
                        Stopped
                    }
                    Ok(MidiSequencerCommand::Stop) => Stopped,
                },
                Playing => match self.control.try_recv() {
                    Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => Playing,
                    Ok(MidiSequencerCommand::Ping) => Playing,
                    Ok(MidiSequencerCommand::Play(pos_us)) => StartPlaying(pos_us),
                    Ok(MidiSequencerCommand::Scale(new_scaling)) => {
                        self.time_control.set_scaling_1000(new_scaling);
                        Playing
                    }
                    Ok(MidiSequencerCommand::SetPosition(pos_us)) => {
                        StartPlaying(pos_us)
                    }
                    Ok(MidiSequencerCommand::SetEvents(events)) => {
                        self.events = events;
                        StartPlaying(0)
                    }
                    Ok(MidiSequencerCommand::Stop) => {
                        self.time_control.stop();
                        for (trk_idx, channel, key) in key_pressed.drain() {
                            let evt = MidiEvent::NoteOff(channel as u8, key, 0);
                            let msg = evt.as_raw(trk_idx, None);
                            conn_out.send(&msg).unwrap();
                        }
                        Stopped
                    }
                },
                StartPlaying(_) => {
                    panic!("StartPlaying should not be reachable here")
                }
            };

            state = match state {
                Stopped => Stopped,
                EOF => EOF,
                StartPlaying(pos_us) => {
                    idx = 0;
                    self.time_control.set_pos_us(pos_us as i64);
                    while idx < self.events.len()
                                && pos_us >= self.events[idx].0 as i64 {
                        idx += 1;
                    }
                    if idx >= self.events.len() {
                        EOF
                    }
                    else {
                        self.time_control.start();
                        Playing
                    }
                }
                Playing => {
                    let pos_us = self.time_control.get_pos_us();
                    while idx < self.events.len()
                                && pos_us >= self.events[idx].0 as i64 {
                        let msg = self.events[idx]
                            .2
                            .as_raw(self.events[idx].1, Some(&mut key_pressed));
                        if msg.len() > 0 {
                            conn_out.send(&msg).unwrap();
                        }
                        idx += 1;
                    }
                    if idx >= self.events.len() {
                        EOF
                    }
                    else {
                        let next_pos = self.events[idx].0 as i64;
                        let opt_sleep_ms = self.time_control.ms_till_pos(next_pos);
                        if let Some(sleep_ms) = opt_sleep_ms {
                            let sleep_ms = sleep_ms.min(20);
                            trace!("sleep {} ms", sleep_ms);
                            sleep(Duration::from_millis(sleep_ms as u64));
                        }
                        Playing
                    }
                }
            }
        }
        conn_out.close();
        trace!("Connection closed");
    }
}

pub struct MidiSequencer {
    time_listener: TimeListener,
    control: mpsc::Sender<MidiSequencerCommand>,
}

impl MidiSequencer {
    pub fn new(out_port: usize) -> MidiSequencer {
        let (tx, rx) = mpsc::channel();
        let controller = TimeController::new();
        let time_listener = controller.new_listener();
        thread::spawn(move || MidiSequencerThread::new(rx, out_port, controller).run());
        MidiSequencer {
            control: tx,
            time_listener,
        }
    }
    pub fn get_new_listener(&self) -> TimeListener {
        self.time_listener.clone()
    }
    pub fn set_pos_us(&self, pos_us: i64) {
        self.control
            .send(MidiSequencerCommand::SetPosition(pos_us))
            .ok();
    }
    pub fn is_finished(&self) -> bool {
        self.control.send(MidiSequencerCommand::Ping).is_err()
    }
    pub fn set_midi_data(&self, events: Vec<RawMidiTuple>) {
        self.control
            .send(MidiSequencerCommand::SetEvents(events))
            .ok();
    }
    pub fn play(&self, pos_us: i64) {
        self.control.send(MidiSequencerCommand::Play(pos_us)).ok();
    }
    pub fn set_scaling_1000(&self, new_scale: u16) {
        self.control
            .send(MidiSequencerCommand::Scale(new_scale))
            .ok();
    }
    pub fn stop(&self) {
        self.control.send(MidiSequencerCommand::Stop).ok();
    }
}
