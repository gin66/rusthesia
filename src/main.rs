use std::thread::sleep;
use std::time::{Duration,Instant};

use log::*;

//mod app;
mod app_control;
mod draw_engine;
mod midi_container;
mod midi_sequencer;
mod scroller;
mod sdl_event_processor;
mod stderrlog;
mod time_controller;
mod usage; // Hacked version of stderrlog crate

/// logging targets defined as abbreviated constants (and avoid typos in repeats)
const EV: &str = &"eventloop";
const SDL: &str = &"sdl";

#[derive(Default)]
struct PerfMonitor<'a> {
    index: usize,
    last_us: u64,
    stamp: Option<Instant>,
    measures: Vec<(u64,u64,u64,usize,&'a str)>,
}
impl<'a> PerfMonitor<'a> {
    pub fn next_loop(&mut self) {
        self.index = 0;
        self.last_us = 0;
        self.stamp = Some(Instant::now());
    }
    pub fn sample(&mut self, name: &'a str) {
        if self.measures.len() <= self.index {
            self.measures.push( (u64::max_value(),0,0,0,name) );
        }
        let elapsed = self.stamp.as_ref().unwrap().elapsed();
        let elapsed_us = elapsed.subsec_micros() as u64;
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
            info!(target: EV,
                  "min={:6.6}us avg={:6.6}us max={:6.6}us {}",
                  us_min,us_sum/ *cnt as u64,us_max,name);
        }
    }
}

fn main() -> Result<(), Box<std::error::Error>> {
    let matches = usage::usage();
    let mut control = app_control::AppControl::from_clap(matches);

    if let Some(modules) = control.is_debug() {
        stderrlog::new()
            .quiet(control.is_quiet())
            .verbosity(control.verbosity())
            .timestamp(stderrlog::Timestamp::Microsecond)
            .modules(modules.iter().cloned().collect::<Vec<String>>())
            .init()
            .unwrap();
    } else {
        stderrlog::new()
            .quiet(control.is_quiet())
            .verbosity(control.verbosity())
            .timestamp(stderrlog::Timestamp::Microsecond)
            .init()
            .unwrap();
    }

    log::set_max_level(match control.verbosity() {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    });

    if control.list_command() {
        return midi_container::list_command(control.is_quiet(), &control.midi_fname());
    }

    let only_midi_player = control.show_tracks().len() == 0;

    control.create_connected_sequencer(only_midi_player)?;
    if only_midi_player {
        let (_, play_events) = app_control::AppControl::read_midi_file(
            &control.midi_fname(),
            control.left_key(),
            control.right_key(),
            control.shift_key(),
            control.show_tracks().clone(),
            control.play_tracks().clone(),
        )?;
        control.play_midi_data(play_events);
        loop {
            sleep(Duration::from_millis(1000));
            if control.seq_is_finished() {
                break;
            }
        }
        return Ok(());
    }

    let nr_of_keys = control.right_key() - control.left_key() + 1;
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    info!(
        target: SDL,
        "display driver: {:?}",
        video_subsystem.current_video_driver()
    );
    info!(target: SDL, "dpi: {:?}", video_subsystem.display_dpi(0));
    info!(
        target: SDL,
        "Screensaver: {:?}",
        video_subsystem.is_screen_saver_enabled()
    );

    info!(
        target: SDL,
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );
    info!(
        target: SDL,
        "{:?}",
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)
    );
    info!(
        target: SDL,
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );

    let window = video_subsystem
        .window(&format!("Rusthesia: {}", control.midi_fname()), 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    info!(target: SDL, "Display Mode: {:?}", window.display_mode());
    //let window_context = window.context();
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .present_vsync()
        .build()?;
    let texture_creator = canvas.texture_creator();
    let mut textures: Vec<sdl2::render::Texture> = vec![];

    let mut event_pump = sdl_context.event_pump().unwrap();
    //event_pump.disable_event(sdl2::event::EventType::Window);

    let mut opt_keyboard: Option<piano_keyboard::Keyboard2d> = None;
    let rows_per_s = 100;
    let waterfall_tex_height = 1000;

    let mut sleep_time_stats = vec![0; 100]; // millisecond resolution
    let mut pf = PerfMonitor::default();

    if false {
    // Measure framerate
    canvas.clear();
    let base_measure = Instant::now();
    for d in vec![50_000,50_000,50_000,
                  40_000,40_000,40_000,
                  30_000,30_000,30_000,
                  20_000,20_000,20_000,
                  10_000,10_000,10_000,
                  5_000,5_000,5_000].into_iter() {
        pf.next_loop();
        let start_measure = Instant::now();
        canvas.clear(); // Blocks for VSYNC after warm up
        pf.sample("canvas clear");
        let elapsed_us_0 = start_measure.elapsed().subsec_micros() as u32;
        sleep(Duration::from_micros(d));
        pf.sample("sleep");
        let elapsed_us_1 = start_measure.elapsed().subsec_micros() as u32;
        canvas.present(); // Blocks for VSYNC after warm up
        pf.sample("canvas present");
        let elapsed_us_2 = start_measure.elapsed().subsec_micros() as u32;
        println!("{}+{}+{}={}",
                    elapsed_us_0,
                    elapsed_us_1-elapsed_us_0,
                    elapsed_us_2-elapsed_us_1,
                    elapsed_us_2);
    }
    pf.output();
    return Ok(());

    // Measure framerate
    canvas.clear();
    canvas.present();
    for d in 0..400 {
        pf.next_loop();
        let start_measure = Instant::now();
        sleep(Duration::from_micros(d*100));
        canvas.present();
        sleep(Duration::from_micros(d*100));
        canvas.present();
        let elapsed_us = start_measure.elapsed().subsec_micros() as u32/2;
        println!("{} framerate={:?} mHz    {} us/frame",d,1_000_000_000/elapsed_us,elapsed_us);
    }
    }

    control.fix_base_time();
    //sequencer.play(-3_000_000);
    'running: loop {
        trace!(target: EV, "at loop start");
        pf.next_loop();

        if control.seq_is_finished() {
            break;
        }
        control.next_loop();
        if control.need_redraw() {
            textures.clear();
        }
        pf.sample("control at loop start");

        let rec = canvas.viewport();
        let width = rec.width();
        let waterfall_overlap = 2 * width / nr_of_keys as u32; // ensure even
        let waterfall_net_height = waterfall_tex_height - waterfall_overlap;

        if opt_keyboard.is_some() {
            if opt_keyboard.as_ref().unwrap().width != width as u16 {
                opt_keyboard = None;
            }
        }
        if opt_keyboard.is_none() {
            trace!("Create Keyboard");
            textures.clear();
            opt_keyboard = Some(
                piano_keyboard::KeyboardBuilder::new()
                    .set_width(rec.width() as u16)?
                    .white_black_gap_present(true)
                    .set_most_left_right_white_keys(control.left_key(), control.right_key())?
                    .build2d(),
            );
        }
        let keyboard = opt_keyboard.as_ref().unwrap();
        if width != keyboard.width as u32 {
            textures.clear();
        }
        pf.sample("keyboard built");

        if textures.len() == 0 {
            trace!("Create keyboard textures");
            // Texture 0 are for unpressed and 1 for pressed keys
            for pressed in vec![false, true].drain(..) {
                let mut texture = texture_creator
                    .create_texture_target(
                        texture_creator.default_pixel_format(),
                        width,
                        keyboard.height as u32,
                    )
                    .unwrap();
                canvas.with_texture_canvas(&mut texture, |tex_canvas| {
                    draw_engine::draw_keyboard(keyboard, tex_canvas, pressed).ok();
                })?;
                textures.push(texture);
            }
        }
        pf.sample("keyboard drawn");

        // Clear canvas
        canvas.set_draw_color(sdl2::pixels::Color::RGB(50, 50, 50));
        canvas.clear();
        pf.sample("canvas cleared");

        // Copy keyboard with unpressed keys
        let dst_rec = sdl2::rect::Rect::new(
            0,
            (rec.height() - keyboard.height as u32 - 1) as i32,
            width,
            keyboard.height as u32,
        );
        canvas.copy(&textures[0], None, dst_rec)?;
        pf.sample("copy keyboard to canvas");

        if control.show_events().is_some() {
            if textures.len() <= 2 {
                // Texture 2.. are for waterfall.
                //
                let maxtime_us = control.show_events().unwrap()[control.show_events_len() - 1].0;
                let rows = (maxtime_us * rows_per_s as u64 + 999_999) / 1_000_000;
                let nr_of_textures =
                    ((rows + waterfall_net_height as u64 - 1) / waterfall_net_height as u64) as u32;
                trace!("Needed rows/textures: {}/{}", rows, nr_of_textures);
                for i in 0..nr_of_textures {
                    let mut texture = texture_creator
                        .create_texture_target(
                            texture_creator.default_pixel_format(),
                            width,
                            waterfall_tex_height,
                        )
                        .unwrap();
                    canvas.with_texture_canvas(&mut texture, |tex_canvas| {
                        draw_engine::draw_waterfall(
                            keyboard,
                            tex_canvas,
                            i,
                            i * waterfall_net_height,
                            waterfall_net_height,
                            waterfall_overlap,
                            rows_per_s,
                            &control.show_events().unwrap(),
                        );
                    })?;
                    textures.push(texture);
                }
            }
        }
        pf.sample("waterfall textures created and drawn");

        let draw_commands = if control.show_events().is_some() {
            let pos_us = control.get_pos_us_at_next_frame();

            let mut draw_commands_1 = draw_engine::get_pressed_key_rectangles(
                &keyboard,
                rec.height() - keyboard.height as u32 - 1,
                pos_us,
                &control.show_events().unwrap());
            let mut draw_commands_2 = draw_engine::copy_waterfall_to_screen(
                textures.len()-2,
                rec.width(),
                rec.height() - keyboard.height as u32,
                waterfall_net_height,
                waterfall_overlap,
                rows_per_s,
                pos_us);
            draw_commands_1.append(&mut draw_commands_2);
            draw_commands_1
        }
        else {
            vec![]
        };
        pf.sample("waterfall and pressed keys commands generated");

        trace!(target: EV, "before drawing to screen");
        for cmd in draw_commands.into_iter() {
            match cmd {
                draw_engine::DrawCommand::CopyToScreen {
                    src_texture, src_rect, dst_rect } => {
                        canvas.copy(&textures[src_texture], src_rect, dst_rect)?;
                    }
            }
        }
        pf.sample("waterfall and pressed keys drawn");

        trace!(target: EV, "before Eventloop");
        let rem_us = loop {
            let rem_us = control.us_till_next_frame();
            if rem_us > 5000 {
                if let Some(event) = event_pump.poll_event() {
                    trace!("event received: {:?}", event);
                    if !sdl_event_processor::process_event(event, &mut control) {
                        break 'running; // Exit loop
                    }
                    continue; // next event
                }
            }
            break rem_us;
        };
        pf.sample("event loop");

        // update stats
        let n = sleep_time_stats.len() - 1;
        sleep_time_stats[(rem_us as usize / 1_000).min(n)] += 1;

        let sleep_duration = Duration::new(0, rem_us as u32 * 1_000);
        trace!(target: EV, "before sleep {:?}", sleep_duration);
        std::thread::sleep(sleep_duration);
        pf.sample("sleep");

        // Sleep until next frame, then present => stable presentation rate
        trace!(target: EV, "before canvas present");
        canvas.present();
        pf.sample("canvas presented");

        control.update_position_if_scrolling();
    }
    sleep(Duration::from_millis(150));

    for i in 0..sleep_time_stats.len() {
        if sleep_time_stats[i] > 0 {
            info!(
                target: EV,
                "Sleep time {:.2} ms - {} times", i, sleep_time_stats[i]
            );
        }
    }
    info!(target: EV, "Lost frames: {}", control.lost_frames_cnt());
    pf.output();
    Ok(())
}

