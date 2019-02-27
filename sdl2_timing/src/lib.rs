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
    base_time: Option<Instant>,
    measured: bool,
    initialized: bool,
    last_frame: u64,
    lost_frames_cnt: usize,
    measures: HashMap<&'a str,(u64,u64,u64,usize)>,
    sleep_time_stats: Vec<u32>, // millisecond resolution
}
impl<'a> Sdl2Timing<'a> {
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
        for (name, (us_min,us_sum,us_max,cnt)) in self.measures.iter() {
            println!(
                  "min={:6.6}us avg={:6.6}us max={:6.6}us {}",
                  us_min,us_sum/ *cnt as u64,us_max,name);
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
        }
        canvas.set_draw_color(color);
        canvas.clear();
        self.sample("Sdl2Timing: after clear");

        self.measured = true;
        if !self.measured {
            for d in vec![50_000,50_000,50_000,
                          40_000,40_000,40_000,
                          30_000,30_000,30_000,
                          20_000,20_000,20_000,
                          10_000,10_000,10_000,
                          5_000,5_000,5_000].into_iter() {
                let start_measure = Instant::now();
                canvas.clear(); // Blocks for VSYNC after warm up
                self.sample("canvas clear");
                let elapsed_us_0 = start_measure.elapsed().subsec_micros() as u32;
                sleep(Duration::from_micros(d));
                self.sample("sleep");
                let elapsed_us_1 = start_measure.elapsed().subsec_micros() as u32;
                canvas.present(); // Blocks for VSYNC after warm up
                self.sample("canvas present");
                let elapsed_us_2 = start_measure.elapsed().subsec_micros() as u32;
                println!("{}+{}+{}={}",
                            elapsed_us_0,
                            elapsed_us_1-elapsed_us_0,
                            elapsed_us_2-elapsed_us_1,
                            elapsed_us_2);
            }
            self.output();

            // Measure framerate
            canvas.clear();
            canvas.present();
            for d in 0..40 {
                let start_measure = Instant::now();
                sleep(Duration::from_micros(d*100));
                canvas.present();
                sleep(Duration::from_micros(d*100));
                canvas.present();
                let elapsed_us = start_measure.elapsed().subsec_micros() as u32/2;
                println!("{} framerate={:?} mHz    {} us/frame",d,1_000_000_000/elapsed_us,elapsed_us);
            }
            let rem_us = 10_000;

            // update stats
            let i = rem_us as usize / 1_000;
            while i >= self.sleep_time_stats.len() {
                self.sleep_time_stats.push(0);
            }
            self.sleep_time_stats[i] += 1;

            let sleep_duration = Duration::new(0, rem_us as u32 * 1_000);
            trace!("before sleep {:?}", sleep_duration);
            std::thread::sleep(sleep_duration);
            self.sample("sleep");

            // Sleep until next frame, then present => stable presentation rate
            trace!("before canvas present");
            canvas.present();
            self.sample("canvas presented");
        }
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
