use crate::midi_sequencer::MidiSequencer;
use crate::midi_sequencer::RawMidiTuple;
use crate::scroller::Scroller;

pub struct AppControl {
    paused: bool,
    scale_1000: u16,
    ms_per_frame: u32,
    pos_us: i64,
    shift_key: i8,
    play_events: Option<Vec<RawMidiTuple>>,
    sequencer: Option<MidiSequencer>,
    scroller: Option<Scroller>,
}
impl AppControl {
    pub fn new() -> AppControl {
        AppControl {
            paused: false,
            scale_1000: 1000,
            ms_per_frame: 40,
            pos_us: 0,
            shift_key: 0,
            play_events: None,
            sequencer: None,
            scroller: None,
        }
    }
}
impl AppControl {
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
            if let Some(play_events) = self.play_events.take() {
                seq.set_midi_data(play_events);
            }
            seq.play(self.pos_us);
            self.sequencer = Some(seq);
        }
        //textures.clear();
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
