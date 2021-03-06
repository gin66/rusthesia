use std::thread::sleep;
use std::time::Duration;

use log::*;

use sdl2_timing::Sdl2Timing;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let nr_displays = video_subsystem.num_video_displays()?;
    for i in 0..nr_displays {
        info!(
            target: SDL,
            "{}: Display Mode: {:?}",
            i,
            video_subsystem.current_display_mode(i)
        );
        info!(
            target: SDL,
            "{}: dpi: {:?}",
            i,
            video_subsystem.display_dpi(i)
        );
    }

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
    let mut st = Sdl2Timing::new_for(&video_subsystem, &window)?;

    //let window_context = window.context();
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .present_vsync()
        .build()?;
    let texture_creator = canvas.texture_creator();
    let mut textures: Vec<sdl2::render::Texture> = vec![];

    let mut event_pump = sdl_context.event_pump().unwrap();

    let rows_per_s = 100;
    let waterfall_tex_height = 1000;

    'running: loop {
        trace!(target: EV, "at loop start");
        let bg_color = sdl2::pixels::Color::RGB(50, 50, 50);
        st.canvas_present_then_clear(&mut canvas, bg_color);

        if control.seq_is_finished() {
            break;
        }
        control.next_loop();

        let rec = canvas.viewport();
        let width = rec.width();
        if control.need_redraw(width as u16) {
            textures.clear();
        }
        st.sample("control at loop start");

        trace!(target: EV, "before Eventloop");
        loop {
            let rem_us = st.us_till_next_frame();
            if rem_us > 5000 {
                if let Some(event) = event_pump.poll_event() {
                    trace!("event received: {:?}", event);
                    if !sdl_event_processor::process_event(event, &mut control) {
                        break 'running; // Exit loop
                    }
                    continue; // next event
                }
            }
            break;
        }
        st.sample("event loop");

        let waterfall_overlap = 2 * width / nr_of_keys as u32; // ensure even
        let waterfall_net_height = waterfall_tex_height - waterfall_overlap;

        if textures.len() == 0 {
            trace!("Create keyboard textures");
            if let Some(keyboard) = control.get_keyboard() {
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
                st.sample("keyboard drawn");
            }
        }

        if textures.len() == 0 {
            continue;
        }

        if let Some(keyboard) = control.get_keyboard() {
            // Copy keyboard with unpressed keys
            let dst_rec = sdl2::rect::Rect::new(
                0,
                (rec.height() - keyboard.height as u32 - 1) as i32,
                width,
                keyboard.height as u32,
            );
            canvas.copy(&textures[0], None, dst_rec)?;
            st.sample("copy keyboard to canvas");
        }

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
                    if let Some(keyboard) = control.get_keyboard() {
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
                    }
                    textures.push(texture);
                }
            }
        }
        st.sample("waterfall textures created and drawn");

        let draw_commands = if control.show_events().is_some() {
            let rem_us = st.us_till_next_frame();
            let pos_us = control.get_pos_us_after(rem_us);

            let mut draw_commands_1 = vec![];
            if let Some(keyboard) = control.get_keyboard() {
                draw_commands_1 = draw_engine::get_pressed_key_rectangles(
                    &keyboard,
                    rec.height() - keyboard.height as u32 - 1,
                    pos_us,
                    &control.show_events().unwrap(),
                );
                let mut draw_commands_2 = draw_engine::copy_waterfall_to_screen(
                    textures.len() - 2,
                    rec.width(),
                    rec.height() - keyboard.height as u32,
                    waterfall_net_height,
                    waterfall_overlap,
                    rows_per_s,
                    pos_us,
                );
                draw_commands_1.append(&mut draw_commands_2);
            }
            draw_commands_1
        } else {
            vec![]
        };
        st.sample("waterfall and pressed keys commands generated");

        trace!(target: EV, "before drawing to screen");
        for cmd in draw_commands.into_iter() {
            match cmd {
                draw_engine::DrawCommand::CopyToScreen {
                    src_texture,
                    src_rect,
                    dst_rect,
                } => {
                    canvas.copy(&textures[src_texture], src_rect, dst_rect)?;
                }
            }
        }
        st.sample("waterfall and pressed keys drawn");

        control.update_position_if_scrolling();
    }
    sleep(Duration::from_millis(150));

    st.output();
    Ok(())
}
