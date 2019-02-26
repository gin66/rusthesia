use log::*;
use std::thread::sleep;
use std::time::{Duration,Instant};

#[derive(Default)]
pub struct Sdl2Timing<'a> {
    index: usize,
    last_us: u64,
    stamp: Option<Instant>,
    ms_per_frame: u32,
    base_time: Option<Instant>,
    measured: bool,
    need_present: bool,
    last_frame: u64,
    lost_frames_cnt: usize,
    measures: Vec<(u64,u64,u64,usize,&'a str)>,
    sleep_time_stats: Vec<u32>, // millisecond resolution
}
impl<'a> Sdl2Timing<'a> {
    pub fn sample(&mut self, name: &'a str) {
        if self.measures.len() <= self.index {
            self.measures.push( (u64::max_value(),0,0,0,name) );
        }
        let elapsed = self.stamp.as_ref().unwrap().elapsed();
        let elapsed_us = elapsed.subsec_micros() as u64 + 1_000_000 * elapsed.as_secs();
        let dt_us = elapsed_us - self.last_us;
        self.last_us = elapsed_us;

        let m = &mut self.measures[self.index];
        m.0 = m.0.min(dt_us);
        m.1 += dt_us;
        m.2 = m.2.max(dt_us);
        m.3 += 1;
        self.index += 1;
    }
    pub fn output(&self) {
        for (us_min,us_sum,us_max,cnt,name) in &self.measures {
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
    pub fn canvas_present_then_clear(&mut self, 
                                     canvas: &mut sdl2::render::Canvas<sdl2::video::Window>) {
        self.index = 0;
        self.last_us = 0;
        self.stamp = Some(Instant::now());

        self.ms_per_frame = 40;

        // Measure framerate
        if self.need_present {
            canvas.present();
        }
        self.need_present = true;
        canvas.set_draw_color(sdl2::pixels::Color::RGB(50, 50, 50));
        canvas.clear();

        if !self.measured {
            self.measured = true;
            let base_measure = Instant::now();
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
