use std::time::{Instant,Duration};

use log::*;
use midly;
use clap::ArgMatches;
use clap::{value_t, values_t};

use crate::midi_sequencer::MidiEvent;
use crate::midi_sequencer::MidiSequencer;
use crate::midi_sequencer::RawMidiTuple;
use crate::midi_container::MidiContainer;
use crate::time_controller::TimeListenerTrait;
use crate::time_controller::TimeListener;
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
    midi_fname: String,
    command_list_tracks: bool,
    quiet: bool,
    debug: Option<Vec<String>>,
    verbose: usize,
    paused: bool,
    scale_1000: u16,
    ms_per_frame: u32,
    base_time: Option<Instant>,
    pos_us: i64,
    left_key: u8,
    right_key: u8,
    shift_key: i8,
    last_frame: u64,
    need_redraw_textures: bool,
    show_tracks: Vec<usize>,
    play_tracks: Vec<usize>,
    show_events: Option<Vec<RawMidiTuple>>,
    sequencer: Option<MidiSequencer>,
    scroller: Scroller,
    pub container: Option<MidiContainer<'a>>,
    time_keeper: Option<TimeListener>,
}
impl<'a> AppControl<'a> {
    #[allow(dead_code)]
    pub fn new() -> AppControl<'a> {
        let scroller = Scroller::new(5_000_000.0);
        AppControl {
            midi_fname: "".to_string(),
            command_list_tracks: false,
            quiet: false,
            debug: None,
            verbose: 0,
            paused: false,
            scale_1000: 1000,
            ms_per_frame: 40,
            base_time: None,
            pos_us: 0,
            left_key: 21,
            right_key: 108,
            shift_key: 0,
            last_frame: 0,
            need_redraw_textures: false,
            show_tracks: vec![],
            play_tracks: vec![],
            show_events: None,
            sequencer: None,
            scroller,
            container: None,
            time_keeper: None,
        }
    }
    pub fn from_clap(matches: ArgMatches) -> AppControl<'a> {
        let quiet = matches.is_present("quiet");
        let debug = if matches.is_present("debug") {
            Some(values_t!(matches.values_of("debug"), String)
                .unwrap_or_else(|_| vec![]))
        }
        else {
            None
        };
        let verbose = matches.occurrences_of("verbose") as usize;
        let shift_key = value_t!(matches, "transpose", i8).unwrap_or_else(|e| e.exit());
        let rd64 = matches.is_present("RD64");
        let (left_key, right_key): (u8, u8) = if rd64 {
            // RD-64 is A1 to C7
            (21 + 12, 108 - 12)
        } else {
            // 88 note piano range from A0 to C8
            (21, 108)
        };
        let midi_fname = matches.value_of("MIDI").unwrap().to_string();
        let list_tracks = matches.is_present("list");
        let show_tracks = values_t!(matches.values_of("show"), usize).unwrap_or_else(|_| vec![]);
        let play_tracks = values_t!(matches.values_of("play"), usize).unwrap_or_else(|e| e.exit());
        let scroller = Scroller::new(5_000_000.0);
        AppControl {
            midi_fname,
            command_list_tracks: list_tracks,
            quiet,
            debug,
            verbose,
            paused: false,
            scale_1000: 1000,
            ms_per_frame: 40,
            base_time: None,
            pos_us: 0,
            left_key,
            right_key,
            shift_key,
            last_frame: 0,
            need_redraw_textures: false,
            show_tracks,
            play_tracks,
            show_events: None,
            sequencer: None,
            scroller,
            container: None,
            time_keeper: None,
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
        if !self.scroller.update_move(y, self.ms_per_frame) {
            if let Some(seq) = self.sequencer.take() {
                seq.stop();
                self.sequencer = Some(seq);
            }
        }
    }
    pub fn finger_touch(&mut self) {
        if self.scroller.stop() && !self.paused {
            if let Some(seq) = self.sequencer.take() {
                seq.play(self.pos_us);
                self.sequencer = Some(seq);
            }
        }
    }
    pub fn finger_up(&mut self) {
        self.scroller.end_move();
    }
    pub fn update_position_if_scrolling(&mut self) {
        if let Some((is_end, delta)) = self.scroller.update_position(self.ms_per_frame) {
            if let Some(seq) = self.sequencer.take() {
                if is_end && !self.paused {
                    seq.play(self.pos_us + delta as i64);
                } else {
                    seq.set_pos_us(self.pos_us + delta as i64);
                }
                self.sequencer = Some(seq);
                self.pos_us = self.pos_us + delta as i64;
            }
        }
    }
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }
    pub fn is_debug(&self) -> Option<&Vec<String>> {
        self.debug.as_ref()
    }
    pub fn verbosity(&self) -> usize {
        self.verbose
    }
    //pub fn shift_key(&self) -> i8 {
    //    self.shift_key
    //}
    pub fn left_key(&self) -> u8 {
        self.left_key
    }
    pub fn right_key(&self) -> u8 {
        self.right_key
    }
    pub fn midi_fname(&self) -> &str {
        &self.midi_fname
    }
    pub fn list_command(&self) -> bool {
        self.command_list_tracks
    }
    pub fn ms_per_frame(&self) -> u32 {
        self.ms_per_frame
    }
    pub fn seq_is_finished(&mut self) -> bool {
        if let Some(seq) = self.sequencer.take() {
            let finished = seq.is_finished();
            self.sequencer = Some(seq);
            finished
        }
        else {
            true
        }
    }
    pub fn show_events(&self) -> Option<&Vec<RawMidiTuple>> {
        self.show_events.as_ref()
    }
    pub fn show_events_len(&self) -> usize {
        self.show_events.as_ref()
            .map(|events| events.len())
            .unwrap_or(0)
    }
    pub fn create_connected_sequencer(&mut self) -> Result<(), Box<std::error::Error>> {
        let mut sequencer = MidiSequencer::new();
        sequencer.connect()?;
        self.time_keeper = Some(sequencer.get_new_listener());
        self.sequencer = Some(sequencer);
        Ok(())
    }
    pub fn fix_base_time(&mut self) {
        self.base_time = Some(Instant::now());
    }
    pub fn get_pos_us_at_next_frame(&mut self) -> i64 {
        if let Some(base_time) = self.base_time.as_ref() {
            let elapsed = base_time.elapsed();
            let elapsed_us = elapsed.subsec_micros();
            let us_per_frame = self.ms_per_frame() * 1_000;
            let rem_us = us_per_frame - elapsed_us % us_per_frame;
            let rem_dur = Duration::new(0, rem_us * 1_000);
            self.time_keeper.as_ref().unwrap().get_pos_us_after(rem_dur)
        }
        else {
            0
        }
    }
    pub fn us_till_next_frame(&mut self) -> u32 {
        if let Some(base_time) = self.base_time.as_ref() {
            let elapsed = base_time.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64
                                + elapsed.as_secs() * 1_000_000;
            let us_per_frame = self.ms_per_frame() as u64 * 1_000;
            let curr_frame = elapsed_us / us_per_frame;
            let lost_frames = curr_frame - self.last_frame;
            self.last_frame = curr_frame;
            if  lost_frames > 1 {
                warn!("{} FRAME(S) LOST",lost_frames - 1);
            }
            (us_per_frame - (elapsed_us -  curr_frame * us_per_frame)) as u32
        }
        else {
            0
        }
    }
    pub fn next_loop(&mut self) {
        self.pos_us = self.time_keeper.as_ref().unwrap().get_pos_us();
    }
    pub fn need_redraw(&mut self) -> bool {
        let need = self.need_redraw_textures;
        self.need_redraw_textures = false;
        need
    }
}

