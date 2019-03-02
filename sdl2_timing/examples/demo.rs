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

    println!("Output size = {:?}",canvas.output_size());
    println!("Logical size = {:?}",canvas.logical_size());
    println!("Viewport = {:?}",canvas.viewport());
    println!("Scale = {:?}",canvas.scale());

    let mut event_pump = sdl_context.event_pump().unwrap();
    //event_pump.disable_event(sdl2::event::EventType::Window);

    let mut x_vals = vec![10;60];
    let bg_color = sdl2::pixels::Color::RGB(50, 50, 50);
    let mut paused = false;
    let mut onestep = false;

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
            for i in 0..x_vals.len() {
                let y = 10+i as i32*12;
                if y as u32 + 10 > height {
                    break;
                }
                let x = x_vals[i];
                if !paused  || onestep {
                    x_vals[i] += i as i32 + 1;
                }
                let bw = 20;
                let subsample: u32 = 8;
                let mut pos_sub_x: u32 = x as u32 % (2*subsample*(width-bw));
                if pos_sub_x > subsample*(width-bw) as u32 {
                    pos_sub_x = 2*subsample*(width-bw) as u32-pos_sub_x;
                }
                let pos_x = (pos_sub_x/subsample) as i32;
                let sub_x = pos_sub_x%subsample;
                // Antialiasing
                let c = (255*(subsample-sub_x)/subsample) as u8;
                if c > 0 {
                    tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(c,c,c));
                    let r = sdl2::rect::Rect::new(
                        pos_x,
                        y,
                        bw,
                        10,
                    );
                    tex_canvas.fill_rect(r).unwrap(); // TODO
                }
                let c = 255-c;
                if c > 0 {
                    tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(c,c,c));
                    let r = sdl2::rect::Rect::new(
                        pos_x+1,
                        y,
                        bw,
                        10,
                    );
                    tex_canvas.fill_rect(r).unwrap(); // TODO
                }
                tex_canvas.set_draw_color(sdl2::pixels::Color::RGB(255,255,255));
                let r = sdl2::rect::Rect::new(
                    pos_x+1,
                    y,
                    bw-1,
                    10,
                );
                tex_canvas.fill_rect(r).unwrap(); // TODO
            }
            onestep = false;
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
                    Event::TextInput {
                        text: ref key,
                        ..
                    } if key == &"n".to_string() => onestep = true,
                    Event::TextInput {
                        text,
                        ..
                    } => println!("Text {:?}", text),
                    raw => println!("{:?}",raw)
                }
            }
            else {
                break;
            }
        };
        let dt_us = st.sample("event loop").0;
        if dt_us > 1500 {
            println!("event loop {} > 1500 us",dt_us);
        }
    }
    sleep(Duration::from_millis(150));

    st.output();
    Ok(())
}

