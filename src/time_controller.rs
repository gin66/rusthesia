use std::time::Instant;
use std::sync::{Arc, Mutex};

struct RefPosition {
    pos_us: i64,
    at_instant: Option<Instant>,
    scaling_1024: u16
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
                let scaled_us =  elapsed_us * self.scaling_1024 as i64 / 1024;
                self.pos_us + scaled_us
            }
        }
    }
    fn advance_to_now(&mut self) {
        let pos_us = self.get_pos_us();
        self.set_pos_us(pos_us);
    }
    pub fn set_scaling_1024(&mut self, new_scale: u16) {
        self.advance_to_now();
        self.scaling_1024 = new_scale;
    }
    pub fn start(&mut self) {
        self.at_instant = Some(Instant::now());
    }
    pub fn is_running(&self) -> bool {
        self.at_instant.is_some()
    }
    pub fn stop(&mut self) {
        self.advance_to_now();
        self.at_instant = None;
    }
}

#[derive(Clone)]
struct TimeController {
    ref_pos: Arc<Mutex<RefPosition>>,
}
impl TimeController {
    pub fn new() -> TimeController {
        TimeController {
            ref_pos: Arc::new(Mutex::new(RefPosition {
            pos_us: 0,
            at_instant: None,
            scaling_1024: 1024
        }))}
    }
    pub fn set_pos_us(&self, pos_us: i64) {
        self.ref_pos.lock().unwrap().set_pos_us(pos_us);
    }
    pub fn get_pos_us(&self) -> i64 {
        self.ref_pos.lock().unwrap().get_pos_us()
    }
    pub fn set_scaling_1024(&self, new_scale: u16) {
        self.ref_pos.lock().unwrap().set_scaling_1024(new_scale);
    }
    pub fn start(&self) {
        self.ref_pos.lock().unwrap().start();
    }
    pub fn is_running(&self) -> bool {
        self.ref_pos.lock().unwrap().is_running()
    }
    pub fn stop(&self) {
        self.ref_pos.lock().unwrap().stop();
    }
}
