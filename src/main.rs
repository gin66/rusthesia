use std::thread::sleep;
use std::time::Duration;

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
        .build()?;
    let texture_creator = canvas.texture_creator();
    let mut textures: Vec<sdl2::render::Texture> = vec![];

    let mut event_pump = sdl_context.event_pump().unwrap();
    //event_pump.disable_event(sdl2::event::EventType::Window);

    let mut opt_keyboard: Option<piano_keyboard::Keyboard2d> = None;
    let rows_per_s = 100;
    let waterfall_tex_height = 1000;

    let mut sleep_time_stats = vec![0; 100]; // millisecond resolution

    control.fix_base_time();
    //sequencer.play(-3_000_000);
    'running: loop {
        trace!(target: EV, "at loop start");
        if control.seq_is_finished() {
            break;
        }
        control.next_loop();
        if control.need_redraw() {
            textures.clear();
        }
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

        // Clear canvas
        canvas.set_draw_color(sdl2::pixels::Color::RGB(50, 50, 50));
        canvas.clear();

        // Copy keyboard with unpressed keys
        let dst_rec = sdl2::rect::Rect::new(
            0,
            (rec.height() - keyboard.height as u32 - 1) as i32,
            width,
            keyboard.height as u32,
        );
        canvas.copy(&textures[0], None, dst_rec)?;

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

            let pos_us = control.get_pos_us_at_next_frame();

            let draw_commands = draw_engine::get_pressed_key_rectangles(
                &keyboard,
                rec.height() - keyboard.height as u32 - 1,
                pos_us,
                &control.show_events().unwrap(),
            );
            for cmd in draw_commands.into_iter() {
                match cmd {
                    draw_engine::DrawCommand::CopyToScreen {
                        src_texture, src_rect, dst_rect } => {
                            canvas.copy(&textures[src_texture], src_rect, dst_rect)?;
                        }
                }
            }
            trace!(target: EV, "before draw_engine::copy_waterfall_to_screen");
            draw_engine::copy_waterfall_to_screen(
                &textures[2..],
                &mut canvas,
                rec.width(),
                rec.height() - keyboard.height as u32,
                waterfall_net_height,
                waterfall_overlap,
                rows_per_s,
                pos_us,
            )?;
        }

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

        // update stats
        let n = sleep_time_stats.len() - 1;
        sleep_time_stats[(rem_us as usize / 1_000).min(n)] += 1;

        let sleep_duration = Duration::new(0, rem_us as u32 * 1_000);
        trace!(target: EV, "before sleep {:?}", sleep_duration);
        std::thread::sleep(sleep_duration);

        // Sleep until next frame, then present => stable presentation rate
        trace!(target: EV, "before canvas present");
        canvas.present();

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
    Ok(())
}
