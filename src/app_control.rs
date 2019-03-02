use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::ArgMatches;
use clap::{value_t, values_t};
use log::*;
use midly;

use crate::midi_container::MidiContainer;
use crate::midi_sequencer::MidiEvent;
use crate::midi_sequencer::MidiSequencer;
use crate::midi_sequencer::RawMidiTuple;
use crate::scroller::Scroller;
use crate::time_controller::TimeListener;
use crate::time_controller::TimeListenerTrait;

const WK: &str = &"worker";

enum WorkerResult {
    EventsLoaded(Result<(Vec<RawMidiTuple>, Vec<RawMidiTuple>), std::io::Error>),
}

enum AppState {
    NeedEventLoading,
    WaitForEventLoading,
    Running,
}

fn transposed_message(
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
                    MidiEvent::NoteOn(channel, shifted_key as u8, pressure.as_int()),
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
                    MidiEvent::NoteOff(channel, shifted_key as u8, pressure.as_int()),
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
                    MidiEvent::Aftertouch(channel, shifted_key as u8, pressure.as_int()),
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
        (midly::MidiMessage::PitchBend(change), true) => {
            Some((time_us, trk, MidiEvent::PitchBend(channel, change.as_int())))
        }
        (midly::MidiMessage::ProgramChange(program), true) => Some((
            time_us,
            trk,
            MidiEvent::ProgramChange(channel, program.as_int()),
        )),
        (_, false) => None,
    }
}

pub struct AppControl {
    state: Option<AppState>,
    midi_fname: String,
    command_list_tracks: bool,
    quiet: bool,
    debug: Option<Vec<String>>,
    verbose: usize,
    paused: bool,
    scale_1000: u16,
    pos_us: i64,
    left_key: u8,
    right_key: u8,
    shift_key: i8,
    need_redraw_textures: bool,
    show_tracks: Vec<usize>,
    play_tracks: Vec<usize>,
    show_events: Option<Vec<RawMidiTuple>>,
    sequencer: Option<MidiSequencer>,
    scroller: Scroller,
    time_keeper: Option<TimeListener>,
    rx: mpsc::Receiver<WorkerResult>,
    tx: mpsc::Sender<WorkerResult>,
    worker: Option<thread::JoinHandle<()>>,
}
impl AppControl {
    #[allow(dead_code)]
    pub fn new() -> AppControl {
        let scroller = Scroller::new(5_000_000.0);
        let (tx, rx) = mpsc::channel();
        AppControl {
            state: None,
            midi_fname: "".to_string(),
            command_list_tracks: false,
            quiet: false,
            debug: None,
            verbose: 0,
            paused: false,
            scale_1000: 1000,
            pos_us: 0,
            left_key: 21,
            right_key: 108,
            shift_key: 0,
            need_redraw_textures: false,
            show_tracks: vec![],
            play_tracks: vec![],
            show_events: None,
            sequencer: None,
            scroller,
            time_keeper: None,
            rx,
            tx,
            worker: None,
        }
    }
    pub fn from_clap(matches: ArgMatches) -> AppControl {
        let (tx, rx) = mpsc::channel();
        let quiet = matches.is_present("quiet");
        let debug = if matches.is_present("debug") {
            Some(values_t!(matches.values_of("debug"), String).unwrap_or_else(|_| vec![]))
        } else {
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
            state: None,
            midi_fname,
            command_list_tracks: list_tracks,
            quiet,
            debug,
            verbose,
            paused: false,
            scale_1000: 1000,
            pos_us: 0,
            left_key,
            right_key,
            shift_key,
            need_redraw_textures: false,
            show_tracks,
            play_tracks,
            show_events: None,
            sequencer: None,
            scroller,
            time_keeper: None,
            rx,
            tx,
            worker: None,
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
                (self.pos_us - 5_000_000).max(-3_000_000)
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
            self.state = Some(AppState::NeedEventLoading);
            self.sequencer = Some(seq);
        }
        self.need_redraw_textures = true;
    }
    pub fn two_finger_scroll_start(&mut self, y: f32) {
        if !self.scroller.update_move(y) {
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
        if let Some((is_end, delta)) = self.scroller.update_position() {
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
    pub fn shift_key(&self) -> i8 {
        self.shift_key
    }
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
    pub fn show_tracks(&self) -> &Vec<usize> {
        &self.show_tracks
    }
    pub fn play_tracks(&self) -> &Vec<usize> {
        &self.play_tracks
    }
    pub fn seq_is_finished(&mut self) -> bool {
        if let Some(seq) = self.sequencer.take() {
            let finished = seq.is_finished();
            self.sequencer = Some(seq);
            finished
        } else {
            true
        }
    }
    pub fn show_events(&self) -> Option<&Vec<RawMidiTuple>> {
        self.show_events.as_ref()
    }
    pub fn show_events_len(&self) -> usize {
        self.show_events
            .as_ref()
            .map(|events| events.len())
            .unwrap_or(0)
    }
    pub fn create_connected_sequencer(
        &mut self,
        exit_on_eof: bool,
    ) -> Result<(), Box<std::error::Error>> {
        let mut sequencer = MidiSequencer::new(exit_on_eof);
        sequencer.connect()?;
        self.time_keeper = Some(sequencer.get_new_listener());
        self.sequencer = Some(sequencer);
        Ok(())
    }
    pub fn get_pos_us_at_next_frame(&mut self) -> i64 {
        //if let Some(base_time) = self.base_time.as_ref() {
        //    let elapsed = base_time.elapsed();
        //    let elapsed_us = elapsed.subsec_micros();
        //    let us_per_frame = self.ms_per_frame() * 1_000;
        //    let rem_us = us_per_frame - elapsed_us % us_per_frame;
        //    let rem_dur = Duration::new(0, rem_us * 1_000);
        //    self.time_keeper.as_ref().unwrap().get_pos_us_after(rem_dur)
        //} else {
        //    0
        //}
        // TODO
        let rem_dur = Duration::new(0, 1 * 1_000);
        self.time_keeper.as_ref().unwrap().get_pos_us_after(rem_dur)
    }
    pub fn read_midi_file(
        midi_fname: &str,
        left_key: u8,
        right_key: u8,
        shift_key: i8,
        show_tracks: Vec<usize>,
        play_tracks: Vec<usize>,
    ) -> Result<(Vec<RawMidiTuple>, Vec<RawMidiTuple>), std::io::Error> {
        let smf_buf = midly::SmfBuffer::open(midi_fname)?;
        let container = MidiContainer::from_buf(&smf_buf)?;
        let show_events = container
            .iter()
            .timed(&container.header().timing)
            .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
            .filter_map(|(time_us, trk, evt)| match evt {
                midly::EventKind::Midi { channel, message } => transposed_message(
                    time_us,
                    trk,
                    channel.as_int(),
                    &message,
                    false,
                    shift_key,
                    left_key,
                    right_key,
                ),
                _ => None,
            })
            .collect::<Vec<_>>();
        let play_events = container
            .iter()
            .timed(&container.header().timing)
            .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
            .filter_map(|(time_us, trk, evt)| match evt {
                midly::EventKind::Midi { channel, message } => transposed_message(
                    time_us,
                    trk,
                    channel.as_int(),
                    &message,
                    true,
                    shift_key,
                    left_key,
                    right_key,
                ),
                _ => None,
            })
            .inspect(|e| trace!("{:?}", e))
            .collect::<Vec<_>>();
        Ok((show_events, play_events))
    }
    pub fn next_loop(&mut self) {
        self.pos_us = self.time_keeper.as_ref().unwrap().get_pos_us();
        let th_result = self.rx.try_recv();
        if th_result.is_ok() {
            trace!(target: WK, "Join worker");
            self.worker
                .take()
                .unwrap()
                .join()
                .expect("something went wrong with worker thread");
            trace!(target: WK, "Join worker done");
        }
        let s = match (self.state.take(), th_result) {
            (Some(_), Ok(WorkerResult::EventsLoaded(Ok((show_events, play_events))))) => {
                trace!(target: WK, "Events loaded");
                self.show_events = Some(show_events);
                if let Some(seq) = self.sequencer.take() {
                    seq.set_midi_data(play_events);
                    seq.play(-3_000_000);
                    self.sequencer = Some(seq);
                }
                AppState::Running
            }
            (Some(AppState::NeedEventLoading), _) => {
                trace!(target: WK, "Start thread for reading the midi file");
                let tx = self.tx.clone();
                let midi_fname = self.midi_fname.clone();
                let left_key = self.left_key;
                let right_key = self.right_key;
                let shift_key = self.shift_key;
                let show_tracks = self.show_tracks.clone();
                let play_tracks = self.play_tracks.clone();
                let jh = thread::spawn(move || {
                    let res = AppControl::read_midi_file(
                        &midi_fname,
                        left_key,
                        right_key,
                        shift_key,
                        show_tracks,
                        play_tracks,
                    );
                    trace!(target: WK, "Send events to main");
                    tx.send(WorkerResult::EventsLoaded(res)).unwrap();
                });
                self.worker = Some(jh);
                AppState::WaitForEventLoading
            }
            (None, _) => AppState::NeedEventLoading,
            (_, _) => AppState::Running,
        };
        self.state = Some(s);
    }
    pub fn need_redraw(&mut self) -> bool {
        let need = self.need_redraw_textures;
        self.need_redraw_textures = false;
        need
    }
    pub fn play_midi_data(&mut self, play_events: Vec<RawMidiTuple>) {
        if let Some(seq) = self.sequencer.take() {
            seq.set_midi_data(play_events);
            seq.play(0);
            self.sequencer = Some(seq);
        }
    }
}
