use log::*;
use std::sync::mpsc;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};

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
            MidiEvent::NoteOn(channel, key,pressure) => 
                                vec![
                                    0x90 + channel,
                                    *key,
                                    *pressure,
                                ],
            MidiEvent::NoteOff(channel, key,pressure) => 
                                vec![
                                    0x80 + channel,
                                    *key,
                                    *pressure,
                                ],
            _ => vec![]
        }
    }
}

pub type RawMidiTuple = (u64, usize, MidiEvent);

enum MidiSequencerCommand {
    Ping,
}

struct MidiSequencerThread {
    out_port: usize,
    control: mpsc::Receiver<MidiSequencerCommand>,
    events: Vec<RawMidiTuple>,
}
impl MidiSequencerThread {
    fn new(
        control: mpsc::Receiver<MidiSequencerCommand>,
        out_port: usize,
        events: Vec<RawMidiTuple>,
    ) -> MidiSequencerThread {
        MidiSequencerThread { out_port, control, events }
    }
    fn run(&mut self) {
        trace!("Opening connection");
        let midi_out = midir::MidiOutput::new("My Test Output").unwrap();
        let mut conn_out = midi_out.connect(self.out_port, "midir-test").unwrap();
        trace!("Connection opened");
        let mut play_position_us = 0;
        let mut idx = 0;
        'main: loop {
            trace!("Loop {} {}",play_position_us,idx);
            let ctl = self.control.try_recv();
            match ctl {
                Err(mpsc::TryRecvError::Empty) => (),
                Err(mpsc::TryRecvError::Disconnected) => break,
                Ok(MidiSequencerCommand::Ping) => (),
            }

            while play_position_us >= self.events[idx].0 {
                trace!("{} {}",play_position_us,self.events[idx].0);
                let msg = self.events[idx].2.as_raw(self.events[idx].1);
                if msg.len() > 0 {
                    conn_out.send(&msg).unwrap();
                }
                idx += 1;
                if idx >= self.events.len() {
                    break 'main;
                }
            }

            let dt_us = (self.events[idx].0 - play_position_us).min(50_000).max(1_000);
            let dt_ms = dt_us/1_000;
            trace!("sleep {} ms",dt_ms);
            sleep(Duration::from_millis(dt_ms));
            play_position_us += dt_ms * 1_000; // not good
        }
        conn_out.close();
        trace!("Connection closed");
    }
}

pub struct MidiSequencer {
    control: mpsc::Sender<MidiSequencerCommand>,
}

impl MidiSequencer {
    pub fn new(out_port: usize, events: Vec<RawMidiTuple>) -> MidiSequencer {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || MidiSequencerThread::new(rx, out_port, events).run());
        MidiSequencer { control: tx }
    }
    pub fn is_finished(&self) -> bool {
        self.control.send(MidiSequencerCommand::Ping).is_err()
    }
}
