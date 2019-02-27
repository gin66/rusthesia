//! # Sdl2Timing
//!
//! ## Purpose   
//!
//! This crate supports on getting the timing right for sdl2 applications.
//! Timing is important to avoid lag (too slow rate) or high cpu load
//! (too high rate). sdl2 offers the possibility to enable vsync synchronization,
//! which is best solution for responsiveness at lowes cpu load.
//! 
//! Example to enable vsync with CanvasBuilder:
//!
//! ```
//!    let mut canvas = sdl2::render::CanvasBuilder::new(window)
//!        .accelerated()
//!        .present_vsync()
//!        .build()?;
//! ```
//!
//! So why need this crate ? 
//! 
//! At least on the macbook air without external monitor, the vsync
//! just is not in use. So depending on vsync for appropriate rate
//! will let the main loop spin at max. rate and creates too high load.
//! With external monitor attached, vsync works. Consequently a solution
//! is needed to either rely on vsync or use delays as fallback.
//!
//! Querying the window.displayMode() for the current framerate,
//! is not reliable. At least on one linux machine 60Hz has been reported,
//! while operating a 4K display at 41Hz.
//!
//! Even relying on vsync is tricky. First canvas.clear() _and_ canvas.present()
//! can wait for vsync to occur. Second for moving element calculation,
//! it is good to know the time till the next frame for proper display position.
//!
//! ## Solution
//!
//! This crate provides a single struct Sdl2Timing, which offers:
//!
//! * Call canvas present and canvas clear
//! * Timing measurement inside the main loop
//! * Output of timing data for development
//! * Info about real framerate,...
//! * Remaining time to next frame

use log::*;
use std::thread::sleep;
use std::time::{Duration,Instant};
use std::collections::HashMap;

use sdl2::render::Canvas;
use sdl2::video::Window;

#[derive(Default)]
pub struct Sdl2Timing<'a> {
    last_us: u64,
    stamp: Option<Instant>,
    ms_per_frame: u32,
    us_per_frame: u32,
    base_time: Option<Instant>,
    measured: bool,
    initialized: bool,
    last_frame: u64,
    has_vsync: bool,
    lost_frames_cnt: usize,
    measures: HashMap<&'a str,(u64,u64,u64,usize)>,
    sleep_time_stats: Vec<u32>, // millisecond resolution
}
impl<'a> Sdl2Timing<'a> {
    fn measure(&mut self, canvas: &mut Canvas<Window>) {
        let start_measure = Instant::now();
        let mut round_us_vec = vec![];
        let mut opt_last_round_us = None;
        loop {
            let elapsed = start_measure.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64
                            + 1_000_000*elapsed.as_secs();
            if elapsed_us > 300_000 {
                break;
            }
            if let Some(last_round_us) = opt_last_round_us {
                let round_us = elapsed_us - last_round_us;
                if round_us > 1_000_000/250 { // Cap at 250 Hz
                    round_us_vec.push(round_us);
                }
            }
            opt_last_round_us = Some(elapsed_us);
            sleep(Duration::from_micros(1_000));
            canvas.present();
        }
        debug!("Measured round times in us: {:?}",round_us_vec);
        if round_us_vec.len() > 5 {
            self.has_vsync = true;
            round_us_vec.sort();
            let median = round_us_vec[round_us_vec.len()/2];
            let upper = median * 21/20;
            let lower = median * 19/20;
            debug!("Median value: {}",median);
            let filtered_round_us = round_us_vec.into_iter()
                                                .filter(|&dt| (dt < upper) && (dt > lower))
                                                .collect::<Vec<_>>();
            debug!("Filtered times +/-5%: {:?}",filtered_round_us);
            self.us_per_frame = (filtered_round_us.iter().cloned().sum::<u64>()
                                        / filtered_round_us.len() as u64) as u32;
            debug!("Calculated frame rate= {:?} us/frame",filtered_round_us);
        }
        self.sample("measured");
    }
    /// It should be called at beginning of the main loop with the display
    /// canvas as argument and the color, which should be used to clear the canvas.
    ///
    pub fn canvas_present_then_clear(&mut self, 
                                     canvas: &mut Canvas<Window>,
                                     color: sdl2::pixels::Color) {
        if self.initialized {
            self.sample("Sdl2Timing: before present and clear");
            canvas.present();
            self.sample("Sdl2Timing: after present, before clear");
        }
        else {
            self.last_us = 0;
            self.stamp = Some(Instant::now());
            self.initialized = true;

            self.ms_per_frame = 40;
            if !self.measured {
                canvas.set_draw_color(color);
                canvas.clear();
                canvas.present();
                canvas.set_draw_color(color);
                canvas.clear();
                self.measure(canvas);
                self.measured = true;
            }
        }
        canvas.set_draw_color(color);
        canvas.clear();
        self.sample("Sdl2Timing: after clear");

        let rem_us = 1_000;
        let sleep_duration = Duration::new(0, rem_us as u32 * 1_000);
        trace!("before sleep {:?}", sleep_duration);
        std::thread::sleep(sleep_duration);
        self.sample("sleep");
    }
    pub fn sample(&mut self, name: &'a str) {
        if !self.measures.contains_key(&name) {
            self.measures.insert(&name, (u64::max_value(),0,0,0));
        }
        let mut m = self.measures.get_mut(&name).unwrap();
        let elapsed = self.stamp.as_ref().unwrap().elapsed();
        let elapsed_us = elapsed.subsec_micros() as u64
                        + 1_000_000 * elapsed.as_secs();
        let dt_us = elapsed_us - self.last_us;
        self.last_us = elapsed_us;

        m.0 = m.0.min(dt_us);
        m.1 += dt_us;
        m.2 = m.2.max(dt_us);
        m.3 += 1;
    }
    pub fn clear(&mut self) {
        self.measures.clear();
    }
    pub fn output(&self) {
        if self.has_vsync {
            println!("VSYNC is in use");
        }
        else {
            println!("VSYNC not detected");
        }
        println!("frame rate= {} us/frame",self.us_per_frame);
        for (name, (us_min,us_sum,us_max,cnt)) in self.measures.iter() {
            println!(
                  "cnt={:6} min={:6.6}us avg={:6.6}us max={:6.6}us {}",
                  cnt,us_min,us_sum/ *cnt as u64,us_max,name);
        }
        for i in 0..self.sleep_time_stats.len() {
            if self.sleep_time_stats[i] > 0 {
                info!(
                    "Sleep time {:.2} ms - {} times", i, self.sleep_time_stats[i]
                );
            }
        }
    //info!(target: EV, "Lost frames: {}", control.lost_frames_cnt());
    }
    pub fn us_till_next_frame(&mut self) -> u32 {
        if let Some(base_time) = self.base_time.as_ref() {
            let elapsed = base_time.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64 + elapsed.as_secs() * 1_000_000;
            let us_per_frame = self.ms_per_frame as u64 * 1_000;
            let curr_frame = elapsed_us / us_per_frame;
            let lost_frames = curr_frame - self.last_frame;
            self.last_frame = curr_frame;
            if lost_frames > 1 {
                warn!("{} FRAME(S) LOST", lost_frames - 1);
                self.lost_frames_cnt += 1;
            }
            (us_per_frame - (elapsed_us - curr_frame * us_per_frame)) as u32
        } else {
            self.base_time = Some(Instant::now());
            0
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
