use std::thread::sleep;
use std::time::{Duration,Instant};

use log::*;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2_timing::Sdl2Timing;

fn main() -> Result<(), Box<std::error::Error>> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    info!(
        "display driver: {:?}",
        video_subsystem.current_video_driver()
    );
    info!("dpi: {:?}", video_subsystem.display_dpi(0));
    info!(
        "Screensaver: {:?}",
        video_subsystem.is_screen_saver_enabled()
    );
    info!(
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );
    info!(
        "{:?}",
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)
    );
    info!(
        "Swap interval: {:?}",
        video_subsystem.gl_get_swap_interval()
    );

    let window = video_subsystem
        .window(&format!("Demo"), 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    info!("Display Mode: {:?}", window.display_mode());
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .present_vsync()
        .build()?;
    let texture_creator = canvas.texture_creator();

    let mut event_pump = sdl_context.event_pump().unwrap();
    //event_pump.disable_event(sdl2::event::EventType::Window);

    let mut pf = Sdl2Timing::default();
    let mut x_vals = vec![10;30];

    'running: loop {
        trace!("at loop start");
        let bg_color = sdl2::pixels::Color::RGB(50, 50, 50);
        pf.canvas_present_then_clear(&mut canvas, bg_color);

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
        canvas.with_texture_canvas(&mut texture, |tex_canvas| {
            tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(0,0,0));
            tex_canvas.clear();
            tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(255,255,255));
            for i in 0..x_vals.len() {
                let x = x_vals[i];
                x_vals[i] += i as i32 + 1;
                let r = sdl2::rect::Rect::new(
                    x / 3 % (width-10) as i32,
                    10+i as i32*20,
                    10,
                    10,
                );
                tex_canvas.fill_rect(r);
            }
        })?;
        canvas.copy(&texture, None, None)?;
        pf.sample("copy keyboard to canvas");

        let rem_us = loop {
            let rem_us = pf.us_till_next_frame();
            if rem_us > 5000 {
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
                        _ => {}
                    }
                    continue; // next event
                }
            }
            break rem_us;
        };
        pf.sample("event loop");
    }
    sleep(Duration::from_millis(150));

    pf.output();
    Ok(())
}

