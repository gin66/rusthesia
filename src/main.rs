use std::io::{stdin, stdout, Write, Read};
use std::thread::sleep;
use std::time::Duration;
use std::fs::File;

use log::LevelFilter;
use simple_logging;

use midir::MidiOutput;
use midly;

use clap::{crate_version,crate_authors,value_t};
use clap::{App,Arg};
use indoc::indoc;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2_unifont::renderer::SurfaceRenderer;
use sdl2::gfx::primitives::DrawRenderer;

#[derive(Copy,Clone)]
enum NoteState {
    Pressed,
    Keep,
    Off
}

// http://www.rwgiangiulio.com/construction/manual/layout.jpg

fn key_to_white(key: u32) -> u32 {
    match key % 12 {
        n @ 0 | n @ 2 | n @ 4 | n @ 5 | n @ 7 | n @ 9 | n @ 11 => (n + 1) / 2 + (key / 12) * 7,
        n @ 1 | n @ 3 | n @ 6 | n @ 8 | n @ 10 => n / 2 + (key / 12) * 7,
        _ => panic!("wrong value"),
    }
}

fn main() -> Result<(), Box<std::error::Error>> {
    simple_logging::log_to_stderr(LevelFilter::Trace);

    let matches = App::new("Rusthesia")
                          .version(crate_version!())
                          .author(crate_authors!("\n"))
                          .about(indoc!("
                                    Reads midi files and creates piano notes waterfall.

                                    Valid key commands, while playing:
                                        <Cursor-Left>   Transpose half tone lower
                                        <Cursor-Right>  Transpose half tone higher
                                        <Cursor-Up>     Go back some time
                                        <Space>         Pause/continue playing
                                        "))
                          .arg(Arg::with_name("transpose")
                               .short("t")
                               .long("transpose")
                               .takes_value(true)
                               .default_value("0")
                               .help("Set number of note steps to transpose"))
                          .arg(Arg::with_name("RD64")
                               .long("rd64")
                               .help("Select 64 key Piano like Roland RD-64"))
                          .arg(Arg::with_name("MIDI")
                               .help("Sets the midi file to use")
                               .required(true)
                               .index(1))
                          .get_matches();

    let mut f = File::open(matches.value_of("MIDI").unwrap())?;
    let mut midi = Vec::new();
    f.read_to_end(&mut midi)?;

    let mut shift_key = value_t!(matches, "transpose", i8).unwrap_or_else(|e| e.exit());

    let rd64 = matches.is_present("rg64");

    // MIDI notes are numbered from 0 to 127 assigned to C-1 to G9
    let (left_key, right_key) = if rd64 {
        // RD-64 is A1 to C7
        (21 + 12, 108 - 12)
    } else {
        // 88 note piano range from A0 to C8
        (21, 108)
    };


    let smf: midly::Smf<Vec<midly::Event>> = midly::Smf::read(&midi).unwrap();
    println!("{:#?}", smf);
    println!("{:#?}", smf.header.timing);
    let tref = match smf.header.timing {
        midly::Timing::Metrical(x) => x.as_int() as u32,
        midly::Timing::Timecode(_x, _y) => 1,
    };

    // Reorder all midi message on timeline
    let mut tracks = vec![];
    tracks.push((0, smf.tracks[1].iter(), None));
    tracks.push((0, smf.tracks[2].iter(), None));
    let mut timeline = vec![(0, vec![], vec![NoteState::Off; 128])];
    loop {
        if tracks.len() > 1 {
            tracks.sort_by_key(|x| u32::max_value() - x.0);
        }
        if let Some((t, mut t_iter, m)) = tracks.pop() {
            let n = timeline.len() - 1;
            if t > timeline[n].0 {
                let note_state = timeline[timeline.len()-1].2.iter()
                                    .map(|ns| match ns {
                                        NoteState::Pressed | NoteState::Keep => NoteState::Keep,
                                        NoteState::Off => NoteState::Off
                                    }).collect::<Vec<_>>();
                timeline.push((t, vec![], note_state));
            }
            let n = timeline.len() - 1;
            timeline[n].1.push(m.clone());
            match m {
                Some(midly::MidiMessage::NoteOn(p1, p2)) => {
                    timeline[n].2[p1.as_int() as usize] = if p2.as_int() > 0 {
                        NoteState::Pressed
                    }
                    else {
                        NoteState::Off 
                    };
                }
                Some(midly::MidiMessage::NoteOff(p1, _p2)) => {
                    timeline[n].2[p1.as_int() as usize] = NoteState::Off;
                }
                m => println!("=> {:#?}", m),
            }
            if let Some(ev) = t_iter.next() {
                let dt = ev.delta.as_int() * tref / 120 / 4;
                if let midly::EventKind::Midi {
                    channel: _c,
                    message: m,
                } = ev.kind
                {
                    tracks.push((t + dt, t_iter, Some(m)));
                } else {
                    println!("=> {:#?}", ev);
                    tracks.push((t + dt, t_iter, None));
                }
            }
        } else {
            break;
        }
    }

    println!("output");
    let midi_out = MidiOutput::new("My Test Output")?;

    // Get an output port (read from console if multiple are available)
    let out_port = match midi_out.port_count() {
        0 => return Err("no output port found".into()),
        1 => {
            println!(
                "Choosing the only available output port: {}",
                midi_out.port_name(0).unwrap()
            );
            0
        }
        _ => {
            println!("\nAvailable output ports:");
            for i in 0..midi_out.port_count() {
                println!("{}: {}", i, midi_out.port_name(i).unwrap());
            }
            print!("Please select output port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            input.trim().parse()?
        }
    };

    println!("\nOpening connection");
    let mut conn_out = midi_out.connect(out_port, "midir-test")?;
    println!("Connection open");
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let into_future_ms = 3_000;

    let mut realtime = 0;
    let mut next_head_pos = 1;
    let mut curr_pos = 0;
    let mut paused = false;
    'running: loop {
        if next_head_pos < timeline.len() {
            if timeline[next_head_pos].0 <= realtime {
                curr_pos = next_head_pos;
                for m in timeline[curr_pos].1.iter() {
                    match m {
                        Some(midly::MidiMessage::NoteOn(p1, p2)) => conn_out
                            .send(&[0x90, (p1.as_int() as i8 - shift_key) as u8, p2.as_int()])
                            .unwrap(),
                        Some(midly::MidiMessage::NoteOff(p1, p2)) => conn_out
                            .send(&[0x80, (p1.as_int() as i8 - shift_key) as u8, p2.as_int()])
                            .unwrap(),
                        m => println!("=> {:#?}", m),
                    }
                }
                next_head_pos += 1;
            }
        } else {
            break;
        }

        canvas.set_draw_color(Color::RGB(100, 100, 100));
        canvas.clear();

        let rec = canvas.viewport();
        let mut black_keys = vec![];
        let mut white_keys = vec![];
        let mut black_keys_on = vec![];
        let mut white_keys_on = vec![];
        let mut traces = vec![];

        let left_white_key = key_to_white(left_key);
        let right_white_key = key_to_white(right_key);
        let nr_white_keys = right_white_key + 1 - left_white_key;

        let white_key_width = rec.width() / nr_white_keys - 1;
        let black_key_width = white_key_width * 11_00 / 22_15;
        let white_key_space = 1;
        let white_key_height = white_key_width * 126_27 / 22_15;
        let black_key_height = white_key_height * 80 / (80 + 45);
        let black_cde_off_center = (13_97 + 11_00 - 22_15) * white_key_width / 22_15;
        let black_fgah_off_center = (13_08 + 11_00 - 22_15) * white_key_width / 22_15;
        let part_width = (white_key_width + white_key_space) * nr_white_keys - white_key_space;
        let offset_x = (rec.left() + rec.right() - part_width as i32) / 2
            - left_white_key as i32 * (white_key_width + white_key_space) as i32;
        let box_rounding = (black_key_width / 2 - 1) as i16;
        for key in left_key..=right_key {
            match key % 12 {
                0 | 2 | 4 | 5 | 7 | 9 | 11 => {
                    let nx = key_to_white(key);
                    let r = sdl2::rect::Rect::new(
                        offset_x + (nx * white_key_width + nx * white_key_space) as i32,
                        rec.bottom() - white_key_height as i32,
                        white_key_width,
                        white_key_height,
                    );
                    traces.push(r.clone());
                    match timeline[curr_pos].2[(key as i8 + shift_key) as usize] {
                        NoteState::Pressed | NoteState::Keep => white_keys_on.push(r),
                        NoteState::Off => white_keys.push(r)
                    }
                }
                1 | 3 | 6 | 8 | 10 => {
                    // black keys
                    let nx = key_to_white(key);
                    let mut left_x = (white_key_width - (black_key_width - white_key_space) / 2
                        + nx * white_key_width
                        + nx * white_key_space) as i32;
                    match key % 12 {
                        1 => left_x -= black_cde_off_center as i32,
                        3 => left_x += black_cde_off_center as i32,
                        6 => left_x -= black_fgah_off_center as i32,
                        10 => left_x += black_fgah_off_center as i32,
                        _ => (),
                    }
                    let r = sdl2::rect::Rect::new(
                        offset_x + left_x,
                        rec.bottom() - white_key_height as i32,
                        black_key_width,
                        black_key_height,
                    );
                    traces.push(r.clone());
                    match timeline[curr_pos].2[(key as i8 + shift_key) as usize] {
                        NoteState::Pressed | NoteState::Keep => black_keys_on.push(r),
                        NoteState::Off => black_keys.push(r)
                    }
                }
                _ => (),
            }
        }

        canvas.set_draw_color(Color::RGB(200, 200, 200));
        canvas.fill_rects(&white_keys).unwrap();
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.fill_rects(&white_keys_on).unwrap();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.fill_rects(&black_keys).unwrap();
        canvas.set_draw_color(Color::RGB(0, 0, 255));
        canvas.fill_rects(&black_keys_on).unwrap();

        let mut pos_to = curr_pos;
        while pos_to < timeline.len() - 1 {
            pos_to += 1;
            if timeline[pos_to].0 > realtime + into_future_ms {
                break;
            }
        }
        for key in left_key..=right_key {
            let y_bottom = rec.bottom() - white_key_height as i32;
            let mut last_y = y_bottom;
            let mut t_rect = traces.remove(0);
            let mut state = NoteState::Off;
            for p in curr_pos..=pos_to {
                let p_t = timeline[p].0;
                let p_t = p_t.max(realtime).min(realtime + into_future_ms);
                let new_y =
                    (realtime + into_future_ms - p_t) as i32 * y_bottom / into_future_ms as i32;
                let new_state = timeline[p].2[(key as i8 + shift_key) as usize];
                match (state,new_state) {
                    (NoteState::Pressed,NoteState::Keep) => (),
                    (NoteState::Pressed,NoteState::Off) |
                    (NoteState::Keep,NoteState::Off) => {
                        t_rect.set_height((last_y - new_y) as u32);
                        t_rect.set_bottom(last_y);
                        canvas.set_draw_color(Color::RGB(0, 255, 255));
                        canvas.rounded_box(
                                t_rect.left() as i16,
                                t_rect.bottom() as i16,
                                t_rect.right() as i16,
                                t_rect.top() as i16,
                                box_rounding,
                                Color::RGB(0,255,255)
                            ).unwrap();
                        last_y = new_y;
                    },
                    (NoteState::Pressed,NoteState::Pressed) |
                    (NoteState::Keep,NoteState::Pressed) => {
                        t_rect.set_height((last_y - new_y - 2) as u32);
                        t_rect.set_bottom(last_y);
                        canvas.set_draw_color(Color::RGB(0, 255, 255));
                        canvas.rounded_box(
                                t_rect.left() as i16,
                                t_rect.bottom() as i16,
                                t_rect.right() as i16,
                                t_rect.top() as i16,
                                box_rounding,
                                Color::RGB(0,255,255)
                            ).unwrap();
                        last_y = new_y;
                    },
                    (NoteState::Keep,NoteState::Keep) => (),
                    (NoteState::Off,NoteState::Keep) | 
                    (NoteState::Off,NoteState::Pressed) |
                    (NoteState::Off,NoteState::Off) => {
                        last_y = new_y;
                    },
                };
                state = new_state;
            }
        }

        let mut renderer =
            SurfaceRenderer::new(Color::RGB(0, 0, 0), Color::RGBA(100, 255, 255, 255));
        renderer.scale = 1;

        let surface = renderer.draw(&format!("{} ms", realtime)).unwrap();
        let demo_tex = texture_creator
            .create_texture_from_surface(surface)
            .unwrap();
        canvas
            .copy(&demo_tex, None, sdl2::rect::Rect::new(10, 10, 100, 20))
            .unwrap();

        let surface = renderer.draw(&format!("shift={}", shift_key)).unwrap();
        let demo_tex = texture_creator
            .create_texture_from_surface(surface)
            .unwrap();
        canvas
            .copy(&demo_tex, None, sdl2::rect::Rect::new(10, 30, 100, 20))
            .unwrap();

        canvas.present();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    ..
                } => {
                    paused = !paused;
                },
                Event::KeyDown {
                    keycode: Some(Keycode::Down),
                    ..
                } => {
                    curr_pos = curr_pos.max(10) - 10;
                    next_head_pos = curr_pos + 1;
                    realtime = timeline[curr_pos].0;
                },
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => shift_key += 1,
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => shift_key -= 1,
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        if next_head_pos < timeline.len() {
            let dt = timeline[next_head_pos].0 - realtime;
            if dt > 0 {
                let dt = dt.min(25);
                sleep(Duration::from_millis(dt as u64));
                if !paused {
                    realtime += dt;
                }
            }
        }
    }
    sleep(Duration::from_millis(150));
    println!("\nClosing connection");
    // This is optional, the connection would automatically be closed as soon as it goes out of scope
    conn_out.close();
    println!("Connection closed");
    Ok(())
}
