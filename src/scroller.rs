// https://stackoverflow.com/questions/4262221/duration-for-kinetic-scrolling-momentum-based-on-velocity
//
use std::time::Instant;

use log::*;

const SCROLL_TIME_MS: u32 = 1_950;
const TIME_CONSTANT_MS: f32 = 10.0;

enum ScrollerState {
    Inactive,
    Scrolling(Instant),
    FreeRunning(Instant),
}

pub struct Scroller {
    state: ScrollerState,
    time_ms: u32,
    start_y: f32,
    last_y: f32,
    scale_factor: f32,
    last_position: f32,
    amplitude: f32,
}

impl Scroller {
    pub fn new(scale_factor: f32) -> Scroller {
        Scroller {
            state: ScrollerState::Inactive,
            time_ms: 0,
            start_y: 0.0,
            last_y: 0.0,
            last_position: 0.0,
            amplitude: 0.0,
            scale_factor,
        }
    }
    pub fn stop(&mut self) -> bool {
        let scrolling = match self.state {
            ScrollerState::FreeRunning(_) => true,
            _ => false,
        };
        self.state = ScrollerState::Inactive;
        scrolling
    }
    pub fn update_move(&mut self, y: f32) -> bool {
        let (state, moving) = match self.state {
            ScrollerState::FreeRunning(_) | ScrollerState::Inactive => {
                trace!("Update move");
                self.start_y = y;
                self.last_y = y;
                self.time_ms = 0;
                self.last_position = y * self.scale_factor;
                self.amplitude = 0.0;
                (ScrollerState::Scrolling(Instant::now()), false)
            }
            ScrollerState::Scrolling(stamp) => {
                self.time_ms = stamp.elapsed().subsec_millis();
                trace!("Update move");
                self.last_y = y;
                let initial_velocity = (y - self.start_y) * 1000.0 / self.time_ms as f32;
                self.amplitude = initial_velocity * self.scale_factor;
                (ScrollerState::Scrolling(stamp), true)
            }
        };
        self.state = state;
        moving
    }
    pub fn end_move(&mut self) {
        self.state = match self.state {
            ScrollerState::Inactive => ScrollerState::Inactive,
            ScrollerState::FreeRunning(_) | ScrollerState::Scrolling(_) => {
                self.time_ms = 0;
                ScrollerState::FreeRunning(Instant::now())
            }
        }
    }
    pub fn update_position(&mut self) -> Option<(bool, f32)> {
        let (state, result) = match self.state {
            ScrollerState::Inactive => (ScrollerState::Inactive, None),
            ScrollerState::Scrolling(stamp) => {
                let new_position = self.last_y * self.scale_factor;
                let delta = new_position - self.last_position;
                self.last_position = new_position;
                trace!("Scroll delta = {}", delta);
                (ScrollerState::Scrolling(stamp), Some((false, delta)))
            }
            ScrollerState::FreeRunning(stamp) => {
                let dt_ms = stamp.elapsed().subsec_millis();
                self.time_ms += dt_ms;
                let delta = self.amplitude / TIME_CONSTANT_MS;
                trace!("Freerunning delta = {}", delta);
                self.amplitude -= delta;
                if self.time_ms < SCROLL_TIME_MS {
                    trace!("time = {}  Scroll delta = {}", self.time_ms, delta);
                    (ScrollerState::FreeRunning(Instant::now()), Some((false, delta)))
                } else {
                    (ScrollerState::Inactive, Some((true, delta)))
                }
            }
        };
        self.state = state;
        result
    }
}
