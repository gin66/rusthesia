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

#[allow(dead_code)]
pub struct Sdl2Timing<'a> {
    last_us: u64,
    stamp: Option<Instant>,
    sdl2_us_per_frame: u32,
    clear_present_avg_us: u32,
    opt_us_per_frame: Option<u32>,
    opt_base_time: Option<Instant>,
    measured: bool,
    initialized: bool,
    has_vsync: bool,
    lost_frames_cnt: usize,
    measures: HashMap<&'a str,(u64,u64,u64,u64)>, // min,sum,max,cnt
    display_mode: sdl2::video::DisplayMode,
    display_name: String,
}
impl<'a> Sdl2Timing<'a> {
    pub fn new_for(
                vs: &'a sdl2::VideoSubsystem,
                win: &Window) -> Result<Sdl2Timing<'a>, String> {
        let display_index = win.display_index()?;
        for i in 0..vs.num_video_displays()? {
            let display_mode = vs.current_display_mode(i)?;
            let display_name = vs.display_name(i)?;
            let selected = if i == display_index {
                " <= displays my window"
            }
            else {
                ""
            };
            info!("Display {} named '{}' uses mode {:?} {}",
                   i,
                   display_name,
                   display_mode,
                   selected);
        }
        let display_mode = vs.current_display_mode(display_index)?;
        let display_name = vs.display_name(display_index)?;
        let sdl2_us_per_frame = 1_000_000 / display_mode.refresh_rate as u32;
        assert!(sdl2_us_per_frame > 0);
        Ok(Sdl2Timing {
            last_us: 0,
            stamp: None,
            sdl2_us_per_frame,
            opt_us_per_frame: None,
            clear_present_avg_us: 0,
            opt_base_time: None,
            measured: false,
            initialized: false,
            has_vsync: false,
            lost_frames_cnt: 0,
            display_mode,
            display_name,
            measures: HashMap::new(),
        })
    }
    pub fn get_us_per_frame(&self) -> u32 {
        if self.has_vsync {
            if let Some(us_per_frame) = self.opt_us_per_frame {
                return us_per_frame;
            }
        }
        self.sdl2_us_per_frame
    }
    fn measure(&mut self, canvas: &mut Canvas<Window>) {
        let start_measure = Instant::now();
        let mut round_us_vec = vec![];
        let mut opt_last_round_us = None;
        let measurement_time_us = 300_000;
        loop {
            let elapsed = start_measure.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64
                            + 1_000_000*elapsed.as_secs();
            if elapsed_us > measurement_time_us {
                break;
            }
            if let Some(last_round_us) = opt_last_round_us {
                let round_us = elapsed_us - last_round_us;
                if round_us > 1_000_000/170 { // Cap at 170 Hz
                    round_us_vec.push(round_us);
                }
            }
            opt_last_round_us = Some(elapsed_us);
            sleep(Duration::from_micros(200));
            self.sample("Sdl2Timing: sleep in measurement");
            canvas.clear();
            let mut avg_us = self.sample("Sdl2Timing: canvas clear").1;
            canvas.present();
            avg_us += self.sample("Sdl2Timing: canvas present").1;
            self.clear_present_avg_us = avg_us;
        }
        debug!("Measured round times in us: {:?}",round_us_vec);
        if round_us_vec.len() > 2 {
            round_us_vec.sort();
            let median = round_us_vec[round_us_vec.len()/2];
            let upper = median * 21/20;
            let lower = median * 19/20;
            debug!("Median value: {}",median);
            let filtered_round_us = round_us_vec.into_iter()
                                                .filter(|&dt| (dt < upper) && (dt > lower))
                                                .collect::<Vec<_>>();
            debug!("Filtered times +/-5%: {:?}",filtered_round_us);
            let sum_round_us: u64 = filtered_round_us.iter().cloned().sum::<u64>();
            let us_per_frame = (sum_round_us / filtered_round_us.len() as u64) as u32;
            debug!("Calculated frame rate= {} us/frame",us_per_frame);
            if sum_round_us > measurement_time_us/3 {
                if us_per_frame > 1_000_000/170 && us_per_frame < 1_000_000/10 {
                    self.has_vsync = true;
                    self.opt_us_per_frame = Some(us_per_frame);
                }
                else {
                    debug!("...outside acceptance window 10..170Hz");
                }
            }
            else {
                debug!("...not enough significant loop times");
            }
        }
    }
    /// It should be called at beginning of the main loop with the display
    /// canvas as argument and the color, which should be used to clear the canvas.
    ///
    pub fn canvas_present_then_clear(&mut self, 
                                     canvas: &mut Canvas<Window>,
                                     color: sdl2::pixels::Color) {
        let mut avg_us = 0;
        if self.initialized {
            if !self.has_vsync {
                let rem_us = self.us_till_next_frame();
                let sleep_duration = Duration::new(0, rem_us as u32 * 1_000);
                std::thread::sleep(sleep_duration);
                self.sample("Sdl2Timing: sleep");
                if let Some(base_time) = self.opt_base_time {
                    let us_per_frame = self.get_us_per_frame();
                    self.opt_base_time = Some(base_time + Duration::new(0, us_per_frame * 1_000));
                }
            }
            else {
                self.opt_base_time = None;
            }
            self.sample("Sdl2Timing: before present and clear");
            canvas.present();
            avg_us += self.sample("Sdl2Timing: canvas present").1;
        }
        else {
            self.last_us = 0;
            self.stamp = Some(Instant::now());
            self.initialized = true;

            if !self.measured {
                // Due to double buffering ensure both buffers are filled
                // with the initial color. Otherwise swapping during
                // measurement would lead to flicker.
                canvas.set_draw_color(color);
                canvas.clear();
                canvas.present();
                canvas.set_draw_color(color);
                canvas.clear();
                self.measure(canvas);
                self.measured = true;
            }
        }
        // TODO: Optional
        canvas.set_draw_color(color);
        canvas.clear();
        if self.opt_base_time.is_none() {
            self.opt_base_time = Some(Instant::now());
        }
        avg_us += self.sample("Sdl2Timing: canvas clear").1;
        self.clear_present_avg_us = avg_us;
    }
    pub fn sample(&mut self, name: &'a str) -> (u32,u32) { // actual, average
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
        (dt_us as u32, (m.1/m.3) as u32)
    }
    pub fn clear(&mut self) {
        self.measures.clear();
    }
    pub fn us_till_next_frame(&mut self) -> u32 {
        if let Some(base_time) = self.opt_base_time.as_ref() {
            let elapsed = base_time.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64 
                                + elapsed.as_secs() * 1_000_000;
            let us_per_frame = self.get_us_per_frame() as u64;
            assert!(us_per_frame > 0);
            if elapsed_us > us_per_frame {
                self.lost_frames_cnt += 1;
                warn!("SYNC MISSED: {}, this time by {} us",self.lost_frames_cnt, elapsed_us - us_per_frame);
                0
            }
            else {
                let rem_us = (us_per_frame - elapsed_us) as u32;
                rem_us.max(1_000)-1_000 //TODO: represents time for clear+present
            }
        } else {
            error!("This path should not be taken");
            self.opt_base_time = Some(Instant::now());
            0
        }
    }
    pub fn us_processing_left(&mut self) -> u32 {
        let us_for_clear_present = self.clear_present_avg_us * 3 / 2;
        self.us_till_next_frame()
            .max(us_for_clear_present) - us_for_clear_present
    }
    pub fn output(&self) {
        if self.has_vsync {
            println!("VSYNC is in use");
            if let Some(us_per_frame) = self.opt_us_per_frame {
                println!("measured frame rate= {} us/frame",us_per_frame);
            }
            else {
                println!("...but no frame rate !?");
            }
        }
        else {
            println!("VSYNC not detected");
        }
        if self.lost_frames_cnt > 0 {
            println!("Lost frame happened: {} times", self.lost_frames_cnt);
        }
        for (name, (us_min,us_sum,us_max,cnt)) in self.measures.iter() {
            println!(
                  "cnt={:6} min={:6.6}us avg={:6.6}us max={:6.6}us {}",
                  cnt,us_min,us_sum/ *cnt as u64,us_max,name);
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
