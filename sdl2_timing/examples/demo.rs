use std::thread::sleep;
use std::time::Duration;

use log::*;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2_timing::Sdl2Timing;

fn main() -> Result<(), Box<std::error::Error>> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    println!(
        "display driver: {:?}",
        video_subsystem.current_video_driver()
    );
    println!("dpi: {:?}", video_subsystem.display_dpi(0));
    println!(
        "Screensaver: {:?}",
        video_subsystem.is_screen_saver_enabled()
    );
    println!(
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );
    println!(
        "{:?}",
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)
    );
    println!(
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );
    let nr_displays = video_subsystem.num_video_displays()?;
    for i in 0..nr_displays {
        println!("{}: Display Mode: {:?}", i, video_subsystem.current_display_mode(i));
    }

    let window = video_subsystem
        .window(&format!("Demo"), 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    let mut st = Sdl2Timing::new_for(&video_subsystem, &window)?;
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .present_vsync()
        .target_texture()
        .build()?;
    let texture_creator = canvas.texture_creator();

    let mut event_pump = sdl_context.event_pump().unwrap();
    //event_pump.disable_event(sdl2::event::EventType::Window);

    let mut x_vals = vec![10;60];
    let bg_color = sdl2::pixels::Color::RGB(50, 50, 50);
    let mut paused = false;

    'running: loop {
        trace!("at loop start");
        st.canvas_present_then_clear(&mut canvas, bg_color);

        let rec = canvas.viewport();
        let width = rec.width();
        let height = rec.height();

        let mut texture = texture_creator
            .create_texture_target(
                texture_creator.default_pixel_format(),
                width,
                height,
            )
            .unwrap();
        st.sample("Create texture");
        canvas.with_texture_canvas(&mut texture, |tex_canvas| {
            tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(0,0,0));
            tex_canvas.clear();
            tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(255,255,255));
            for i in 0..x_vals.len() {
                let x = x_vals[i];
                if !paused {
                    x_vals[i] += i as i32 + 1;
                }
                let bw = 20;
                let mut pos_x = x / 4 % (2*(width-bw)) as i32;
                if pos_x > (width-bw) as i32 {
                    pos_x = 2*(width-bw) as i32-pos_x;
                }
                let r = sdl2::rect::Rect::new(
                    pos_x,
                    10+i as i32*12,
                    bw,
                    10,
                );
                tex_canvas.fill_rect(r).unwrap(); // TODO
            }
        })?;
        canvas.copy(&texture, None, None)?;
        st.sample("Draw content to canvas");

        loop {
            if st.us_processing_left() < 2_000 {
                break;
            }
            if let Some(event) = event_pump.poll_event() {
                println!("event received: {:?}", event);
                match event {
                    Event::Window { win_event, .. } => {
                        trace!("Unprocessed window Event: {:?}", win_event);
                    }
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,
                    Event::KeyDown {
                        keycode: Some(Keycode::Delete),
                        ..
                    } => st.clear(),
                    Event::KeyDown {
                        keycode: Some(Keycode::Space),
                        ..
                    } => paused ^= true,
                    _ => {}
                }
            }
            else {
                break;
            }
        };
        let dt_us = st.sample("event loop").0;
        if dt_us > 500 {
            println!("event loop {} > 500 us",dt_us);
        }
    }
    sleep(Duration::from_millis(150));

    st.output();
    Ok(())
}

