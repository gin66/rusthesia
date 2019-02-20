use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub struct RefPosition {
    pos_us: i64,
    at_instant: Option<Instant>,
    scaling_1000: u16,
}
impl RefPosition {
    pub fn set_pos_us(&mut self, pos_us: i64) {
        self.pos_us = pos_us;
        if self.at_instant.is_some() {
            self.at_instant = Some(Instant::now());
        }
    }
    pub fn get_pos_us(&self) -> i64 {
        match self.at_instant.as_ref() {
            None => self.pos_us,
            Some(instant) => {
                let elapsed = instant.elapsed();
                let mut elapsed_us = elapsed.subsec_micros() as i64;
                elapsed_us += elapsed.as_secs() as i64 * 1_000_000;
                let scaled_us = elapsed_us * self.scaling_1000 as i64 / 1000;
                self.pos_us + scaled_us
            }
        }
    }
    pub fn get_pos_us_after(&self, duration: Duration) -> i64 {
        match self.at_instant.as_ref() {
            None => self.pos_us,
            Some(instant) => {
                let elapsed = instant.elapsed() + duration;
                let mut elapsed_us = elapsed.subsec_micros() as i64;
                elapsed_us += elapsed.as_secs() as i64 * 1_000_000;
                let scaled_us = elapsed_us * self.scaling_1000 as i64 / 1000;
                self.pos_us + scaled_us
            }
        }
    }
    fn advance_to_now(&mut self) {
        let pos_us = self.get_pos_us();
        self.set_pos_us(pos_us);
    }
    pub fn set_scaling_1000(&mut self, new_scale: u16) {
        self.advance_to_now();
        self.scaling_1000 = new_scale;
    }
    pub fn start(&mut self) {
        self.at_instant = Some(Instant::now());
    }
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.at_instant.is_some()
    }
    pub fn stop(&mut self) {
        self.advance_to_now();
        self.at_instant = None;
    }
    pub fn ms_till_pos(&self, next_pos_us: i64) -> Option<u32> {
        let pos_us = self.get_pos_us();
        if pos_us > next_pos_us {
            None
        } else {
            let rem_us = next_pos_us - pos_us;
            let scaled_ms = rem_us / self.scaling_1000 as i64;
            if scaled_ms == 0 {
                None
            } else {
                Some(scaled_ms as u32)
            }
        }
    }
}

pub trait TimeListenerTrait {
    fn get_locked(&self) -> Option<MutexGuard<RefPosition>>;
    fn get_pos_us(&self) -> i64 {
        self.get_locked().unwrap().get_pos_us()
    }
    fn get_pos_us_after(&self, duration: Duration) -> i64 {
        self.get_locked().unwrap().get_pos_us_after(duration)
    }
    fn is_running(&self) -> bool {
        self.get_locked().unwrap().is_running()
    }
    fn ms_till_pos(&self, next_pos_us: i64) -> Option<u32> {
        self.get_locked().unwrap().ms_till_pos(next_pos_us)
    }
}

#[derive(Clone)]
pub struct TimeListener {
    ref_pos: Arc<Mutex<RefPosition>>,
}
impl TimeListenerTrait for TimeListener {
    fn get_locked(&self) -> Option<MutexGuard<RefPosition>> {
        self.ref_pos.lock().ok()
    }
}

pub struct TimeController {
    ref_pos: Arc<Mutex<RefPosition>>,
}
impl TimeController {
    pub fn new() -> TimeController {
        TimeController {
            ref_pos: Arc::new(Mutex::new(RefPosition {
                pos_us: 0,
                at_instant: None,
                scaling_1000: 1000,
            })),
        }
    }
    pub fn new_listener(&self) -> TimeListener {
        TimeListener {
            ref_pos: self.ref_pos.clone(),
        }
    }
    pub fn set_pos_us(&self, pos_us: i64) {
        self.get_locked().unwrap().set_pos_us(pos_us);
    }
    pub fn set_scaling_1000(&self, new_scale: u16) {
        self.get_locked().unwrap().set_scaling_1000(new_scale);
    }
    pub fn start(&self) {
        self.get_locked().unwrap().start();
    }
    pub fn stop(&self) {
        self.get_locked().unwrap().stop();
    }
}
impl TimeListenerTrait for TimeController {
    fn get_locked(&self) -> Option<MutexGuard<RefPosition>> {
        self.ref_pos.lock().ok()
    }
}
