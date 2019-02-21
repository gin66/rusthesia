use log::*;

use midir::MidiOutput;
use midly;

use crate::midi_sequencer::MidiEvent;
use crate::midi_sequencer::MidiSequencer;
use crate::midi_sequencer::RawMidiTuple;
use crate::midi_container::MidiContainer;
use crate::scroller::Scroller;

pub fn transposed_message(
    time_us: u64,
    trk: usize,
    channel: u8,
    message: &midly::MidiMessage,
    all: bool,
    shift_key: i8,
    left_key: u8,
    right_key: u8,
) -> Option<(u64, usize, MidiEvent)> {
    match (message, all) {
        (midly::MidiMessage::NoteOn(key, pressure), _) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            } else {
                Some((
                    time_us,
                    trk,
                    MidiEvent::NoteOn(
                        0 * channel,
                        shifted_key as u8,
                        pressure.as_int(),
                    ),
                ))
            }
        }
        (midly::MidiMessage::NoteOff(key, pressure), _) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            } else {
                Some((
                    time_us,
                    trk,
                    MidiEvent::NoteOff(
                        0 * channel,
                        shifted_key as u8,
                        pressure.as_int(),
                    ),
                ))
            }
        }
        (midly::MidiMessage::Aftertouch(key, pressure), true) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            } else {
                Some((
                    time_us,
                    trk,
                    MidiEvent::Aftertouch(
                        0 * channel,
                        shifted_key as u8,
                        pressure.as_int(),
                    ),
                ))
            }
        }
        (midly::MidiMessage::Controller(control, value), true) => Some((
            time_us,
            trk,
            MidiEvent::Controller(channel, control.as_int(), value.as_int()),
        )),
        (midly::MidiMessage::ChannelAftertouch(pressure), true) => Some((
            time_us,
            trk,
            MidiEvent::ChannelAftertouch(channel, pressure.as_int()),
        )),
        (midly::MidiMessage::PitchBend(change), true) => Some((
            time_us,
            trk,
            MidiEvent::PitchBend(channel, change.as_int()),
        )),
        (midly::MidiMessage::ProgramChange(program), true) => Some((
            time_us,
            trk,
            MidiEvent::ProgramChange(channel, program.as_int()),
        )),
        (_, false) => None,
    }
}

pub struct AppControl<'a> {
    paused: bool,
    scale_1000: u16,
    ms_per_frame: u32,
    pos_us: i64,
    left_key: u8,
    right_key: u8,
    shift_key: i8,
    need_redraw_textures: bool,
    show_tracks: Vec<usize>,
    play_tracks: Vec<usize>,
    show_events: Option<Vec<RawMidiTuple>>,
    sequencer: Option<MidiSequencer>,
    scroller: Option<Scroller>,
    container: Option<MidiContainer<'a>>,
}
impl<'a> AppControl<'a> {
    pub fn new() -> AppControl<'a> {
        AppControl {
            paused: false,
            scale_1000: 1000,
            ms_per_frame: 40,
            pos_us: 0,
            left_key: 0,
            right_key: 0,
            shift_key: 0,
            need_redraw_textures: false,
            show_tracks: vec![],
            play_tracks: vec![],
            show_events: None,
            sequencer: None,
            scroller: None,
            container: None,
        }
    }
    pub fn toggle_play(&mut self) {
        self.paused = !self.paused;
        if let Some(seq) = self.sequencer.take() {
            if self.paused {
                seq.stop();
            } else {
                seq.play(self.pos_us);
            }
            self.sequencer = Some(seq);
        }
    }
    pub fn modify_scaling(&mut self, increase: bool) {
        if let Some(seq) = self.sequencer.take() {
            self.scale_1000 = if increase {
                4000.min(self.scale_1000 + 50)
            } else {
                250.max(self.scale_1000 - 50)
            };
            seq.set_scaling_1000(self.scale_1000);
            self.sequencer = Some(seq);
        }
    }
    pub fn change_position(&mut self, forward: bool) {
        if let Some(seq) = self.sequencer.take() {
            self.pos_us = if forward {
                self.pos_us + 5_000_000
            } else {
                if self.pos_us > 5_000_000 {
                    self.pos_us - 5_000_000
                } else {
                    0
                }
            };
            if self.paused {
                seq.set_pos_us(self.pos_us);
            } else {
                seq.play(self.pos_us);
            }
            self.sequencer = Some(seq);
        }
    }
    pub fn tune_up(&mut self, tune_up: bool) {
        self.shift_key = if tune_up {
            self.shift_key.min(126) + 1
        } else {
            self.shift_key.max(-126) - 1
        };
        if let Some(seq) = self.sequencer.take() {
            seq.stop();
            if let Some(container) = self.container.take() {
                let show_events = container
                    .iter()
                    .timed(&container.header().timing)
                    .filter(|(_time_us, trk, _evt)| self.show_tracks.contains(trk))
                    .filter_map(|(time_us, trk, evt)| match evt {
                        midly::EventKind::Midi { channel, message } => {
                            transposed_message(
                                time_us,
                                trk,
                                channel.as_int(),
                                &message,
                                false,
                                self.shift_key,
                                self.left_key,
                                self.right_key)
                        },
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                self.show_events = Some(show_events);
                let play_events = container
                    .iter()
                    .timed(&container.header().timing)
                    .filter(|(_time_us, trk, _evt)| self.play_tracks.contains(trk))
                    .filter_map(|(time_us, trk, evt)| match evt {
                        midly::EventKind::Midi { channel, message } => {
                            transposed_message(
                                time_us,
                                trk,
                                channel.as_int(),
                                &message,
                                true,
                                self.shift_key,
                                self.left_key,
                                self.right_key)
                        },
                        _ => None,
                    })
                    .inspect(|e| trace!("{:?}", e))
                    .collect::<Vec<_>>();
                seq.set_midi_data(play_events);
                self.container = Some(container);
            }
            seq.play(self.pos_us);
            self.sequencer = Some(seq);
        }
        self.need_redraw_textures = true;
    }
    pub fn two_finger_scroll_start(&mut self, y: f32) {
        if let Some(mut scr) = self.scroller.take() {
            if !scr.update_move(y, self.ms_per_frame) {
                if let Some(seq) = self.sequencer.take() {
                    seq.stop();
                    self.sequencer = Some(seq);
                }
            };
            self.scroller = Some(scr);
        }
    }
    pub fn finger_touch(&mut self) {
        if let Some(mut scr) = self.scroller.take() {
            if scr.stop() && !self.paused {
                if let Some(seq) = self.sequencer.take() {
                    seq.play(self.pos_us);
                    self.sequencer = Some(seq);
                }
            };
            self.scroller = Some(scr);
        }
    }
    pub fn finger_up(&mut self) {
        if let Some(mut scr) = self.scroller.take() {
            scr.end_move();
            self.scroller = Some(scr);
        }
    }
}
