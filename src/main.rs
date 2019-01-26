use std::thread::sleep;
use std::time::Duration;
use std::io::{stdin, stdout, Write};
use std::error::Error;

use simple_logging;
use log::LevelFilter;

use midly;
use midir::MidiOutput;

use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err.description())
    }
}

fn run() -> Result<(), Box<Error>> {
    simple_logging::log_to_stderr(LevelFilter::Trace);

    let midi = include_bytes!("../Forrest Gump_Feather Theme.mid");
    let smf: midly::Smf<Vec<midly::Event>>=midly::Smf::read(midi).unwrap();
    println!("{:#?}", smf);

    let midi_out = MidiOutput::new("My Test Output")?;
    
    // Get an output port (read from console if multiple are available)
    let out_port = match midi_out.port_count() {
        0 => return Err("no output port found".into()),
        1 => {
            println!("Choosing the only available output port: {}", midi_out.port_name(0).unwrap());
            0
        },
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
    println!("{:#?}",smf.header.timing);
    let tref = match smf.header.timing {
        midly::Timing::Metrical(x) => x.as_int() as u32,
        midly::Timing::Timecode(x,y) => 1
    };
    let mut tracks = vec![];
    tracks.push( (0,smf.tracks[1].iter(), None) );
    tracks.push( (0,smf.tracks[2].iter(), None) );

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
 
    let window = video_subsystem.window("rust-sdl2 demo", 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
 
    let mut canvas = window.into_canvas().build().unwrap();
 
    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut i = 0;

    let mut realtime = 0;
    'running: loop {
        canvas.set_draw_color(Color::RGB(i, 64, 255 - i));
        canvas.clear();

        i = (i + 1) % 255;
        let rec = canvas.viewport();
        let mut black_keys = vec![];
        let mut white_keys = vec![];

        let white_key_width = rec.width() / 7 / 5 - 1;
        let black_key_width = white_key_width * 5/7;
        let white_key_space = 1;
        let white_key_height = rec.height() / 5;
        let black_key_height = white_key_height * 2 / 3;
        let part_width = (white_key_width+white_key_space) * (7*5) - white_key_space;
        let offset_x = rec.left() + (rec.width() - part_width) as i32 / 2;
        for key in 0..60 {
            match key % 12 {
                n @ 0 | n @ 2 | n @ 4 | n @ 5 | n @ 7 | n @ 9 | n @ 11 => {
                    let nx = (n+1) / 2 + (key/12)*7;
                    let r = sdl2::rect::Rect::new(
                        offset_x + (nx * white_key_width + nx * white_key_space) as i32,
                        rec.bottom()-white_key_height as i32,
                        white_key_width,
                        white_key_height
                        );
                    white_keys.push(r);
                },
                n @ 1 | n @ 3 | n @ 6 | n @ 8 | n @ 10 => {
                    let nx = n / 2 + (key/12)*7;
                    let r = sdl2::rect::Rect::new(
                        offset_x + (white_key_width - (black_key_width-white_key_space)/2
                                        + nx * white_key_width + nx * white_key_space) as i32,
                        rec.bottom()-white_key_height as i32,
                        black_key_width,
                        black_key_height
                        );
                    black_keys.push(r);
                },
                _ => ()
            }
        }
        
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.fill_rects(&white_keys);
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.fill_rects(&black_keys);

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        canvas.present();

        if tracks.len() > 1 {
            tracks.sort_by_key(|x| u32::max_value()-x.0);
        }
        if let Some( (t,mut t_iter, m) ) = tracks.pop() {
            if t > realtime {
                sleep(Duration::from_millis((t-realtime) as u64));
                realtime = t;
            }
            match m {
                Some(midly::MidiMessage::NoteOn(p1,p2)) =>
                    conn_out.send(&[0x90, p1.as_int(), p2.as_int()]).unwrap(),
                Some(midly::MidiMessage::NoteOff(p1,p2)) =>
                    conn_out.send(&[0x80, p1.as_int(), p2.as_int()]).unwrap(),
                m => 
                    println!("=> {:#?}",m)
            }
            if let Some(ev) = t_iter.next() {
                let dt = ev.delta.as_int() * tref / 120 / 4;
                println!("dt={} ms",dt);
                if let midly::EventKind::Midi{channel: _c,message: m} = ev.kind {
                    tracks.push( (t+dt,t_iter, Some(m)) );
                }
                else {
                    println!("=> {:#?}",ev);
                    tracks.push( (t+dt,t_iter, None) );
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
