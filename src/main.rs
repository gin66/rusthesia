use std::io::{stdin, stdout, Write};
use std::thread::sleep;
use std::time::Duration;

use log::*;
use simple_logging;

use midir::MidiOutput;
use midly;

use clap::{value_t, values_t};

use sdl2::keyboard::Keycode;
use sdl2::event::Event;
use sdl2::gfx::framerate::FPSManager;

mod time_controller;
mod midi_container;
mod midi_sequencer;
mod draw_engine;
mod usage;

#[derive(Copy, Clone)]
enum NoteState {
    Pressed(usize),
    Keep(usize),
    Off,
}


fn main() -> Result<(), Box<std::error::Error>> {
    let matches = usage::usage();

    let debug = matches.is_present("debug");
    simple_logging::log_to_stderr(if debug {
        LevelFilter::Trace
    } else {
        LevelFilter::Error
    });

    let mut shift_key = value_t!(matches, "transpose", i8).unwrap_or_else(|e| e.exit());

    // MIDI notes are numbered from 0 to 127 assigned to C-1 to G9
    let rd64 = matches.is_present("RD64");
    let (left_key, right_key) = if rd64 {
        // RD-64 is A1 to C7
        (21 + 12, 108 - 12)
    } else {
        // 88 note piano range from A0 to C8
        (21, 108)
    };

    let midi_fname = matches.value_of("MIDI").unwrap();
    let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
    let container = midi_container::MidiContainer::from_buf(&smf_buf)?;
    if debug {
        for _evt in container.iter() {
            //trace!("{:?}", evt);
        }
        for evt in container.iter().timed(&container.header().timing) {
            trace!("timed: {:?}", evt);
        }
    }

    let list_tracks = matches.is_present("list");
    if list_tracks {
        for i in 0..container.nr_of_tracks() {
            println!("Track {}:", i);
            let mut used_channels = vec![false; 16];
            for evt in container.iter().filter(|e| e.1 == i) {
                match evt.2 {
                    midly::EventKind::Midi {
                        channel: c,
                        message: _m,
                    } => {
                        used_channels[c.as_int() as usize] = true;
                    }
                    midly::EventKind::SysEx(_) => (),
                    midly::EventKind::Escape(_) => (),
                    midly::EventKind::Meta(mm) => match mm {
                        midly::MetaMessage::Text(raw) => {
                            println!("  Text: {}", String::from_utf8_lossy(raw));
                        }
                        midly::MetaMessage::ProgramName(raw) => {
                            println!("  Program name: {}", String::from_utf8_lossy(raw));
                        }
                        midly::MetaMessage::DeviceName(raw) => {
                            println!("  Device name: {}", String::from_utf8_lossy(raw));
                        }
                        midly::MetaMessage::InstrumentName(raw) => {
                            println!("  Instrument name: {}", String::from_utf8_lossy(raw));
                        }
                        midly::MetaMessage::TrackName(raw) => {
                            println!("  Track name: {}", String::from_utf8_lossy(raw));
                        }
                        midly::MetaMessage::MidiChannel(channel) => {
                            println!("  Channel: {}", channel.as_int());
                        }
                        midly::MetaMessage::Tempo(ms_per_beat) => {
                            trace!("  Tempo: {:?}", ms_per_beat);
                        }
                        midly::MetaMessage::EndOfTrack => (),
                        mm => warn!("Not treated meta message: {:?}", mm),
                    },
                }
            }
            println!(
                "  Used channels: {:?}",
                used_channels
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v)
                    .map(|(c, _)| c)
                    .collect::<Vec<_>>()
            );
        }
        return Ok(());
    }

    let show_tracks = values_t!(matches.values_of("show"), usize).unwrap_or_else(|_| vec![]);;
    let play_tracks = values_t!(matches.values_of("play"), usize).unwrap_or_else(|e| e.exit());;

    // Get all the events to show/play
    let show_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
        .filter_map(|(time_us, trk, evt)| match evt {
            midly::EventKind::Midi { channel, message } => match message {
                midly::MidiMessage::NoteOn(key, pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOn(channel.as_int(), key.as_int(), pressure.as_int()),
                )),
                midly::MidiMessage::NoteOff(key, pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOff(channel.as_int(), key.as_int(), pressure.as_int()),
                )),
                _ => None
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    let play_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
        .filter_map(|(time_us, trk, evt)| match evt {
            midly::EventKind::Midi { channel, message } => match message {
                // TODO: CHANNEL HACK REMOVAL
                midly::MidiMessage::NoteOn(key, pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOn(0*channel.as_int(), key.as_int(), pressure.as_int()),
                )),
                midly::MidiMessage::NoteOff(key, pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOff(0*channel.as_int(), key.as_int(), pressure.as_int()),
                )),
                midly::MidiMessage::Aftertouch(key, pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::Aftertouch(channel.as_int(), key.as_int(), pressure.as_int()),
                )),
                midly::MidiMessage::Controller(control, value) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::Controller(channel.as_int(), control.as_int(), value.as_int()),
                )),
                midly::MidiMessage::ChannelAftertouch(pressure) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::ChannelAftertouch(channel.as_int(), pressure.as_int()),
                )),
                midly::MidiMessage::PitchBend(change) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::PitchBend(channel.as_int(), change.as_int()),
                )),
                midly::MidiMessage::ProgramChange(program) => Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::ProgramChange(channel.as_int(), program.as_int()),
                )),
                _ => None
            },
            _ => None,
        })
        .inspect(|e| trace!("{:?}",e))
        .collect::<Vec<_>>();

    trace!("output");
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
    drop(midi_out);

    let sequencer = midi_sequencer::MidiSequencer::new(out_port, play_events);

    if show_tracks.len() == 0 {
        sequencer.play(0, Some(1000), None);
        loop {
            sleep(Duration::from_millis(1000));
            if sequencer.is_finished() {
                break;
            }
        }
        return Ok(());
    }

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window(&format!("Rusthesia: {}", midi_fname), 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    let mut window_context = window.context();
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .build()?;
    let mut texture_creator = canvas.texture_creator();
    let mut textures: Vec<sdl2::render::Texture> = vec![];

    let draw_engine = draw_engine::DrawEngine::init(video_subsystem)?;
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut paused = false;
    let mut scale_1000 = 1000;
    let mut fps_manager = FPSManager::new();
    let mut opt_keyboard = None;

    fps_manager.set_framerate(50)?;

    sequencer.play(0, Some(scale_1000), None);
    'running: loop {
        let pos_us: i64 = sequencer.pos_us();

        let rec = canvas.viewport();
        let width = rec.width();

        if opt_keyboard.is_none() {
            trace!("Create Keyboard");
            textures.clear();
            opt_keyboard = Some(piano_keyboard::KeyboardBuilder::new()
                                .set_width(rec.width() as u16)?
                                .white_black_gap_present(true)
                                .build2d());
        }
        let keyboard = opt_keyboard.as_ref().unwrap();
        if width != keyboard.width as u32 {
            textures.clear();
        }

        if textures.len() == 0 {
            trace!("Create keyboard textures");
            // Texture 0 are for unpressed and 1 for pressed keys
            for pressed in vec![false,true].drain(..) {
                let mut texture = texture_creator
                    .create_texture_target(texture_creator.default_pixel_format(),
                                            width, keyboard.height as u32)
                    .unwrap();
                let result = canvas.with_texture_canvas(&mut texture, |texture_canvas| {
                    draw_engine::draw_keyboard(keyboard,texture_canvas,pressed);
                });
                textures.push(texture);
            }
        }

        // Clear canvas
        canvas.set_draw_color(sdl2::pixels::Color::RGB(100,100,100));
        canvas.clear();

        // Copy keyboard with unpressed keys
        let dst_rec = sdl2::rect::Rect::new(0,(rec.height()-keyboard.height as u32-1) as i32,
                                            width,keyboard.height as u32);
        canvas.copy(&textures[0], None, dst_rec)?;

        let pressed_rectangles = draw_engine::get_pressed_key_rectangles(&keyboard,
                                            rec.height()-keyboard.height as u32 - 1,
                                            pos_us, &show_events);
        for (src_rec,dst_rec) in pressed_rectangles.into_iter() {
            canvas.copy(&textures[1], src_rec, dst_rec)?;
        }

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
                    if paused {
                        sequencer.stop();
                    }
                    else {
                        sequencer.play(pos_us, None, None);
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Plus),
                    ..
                } => {
                    scale_1000 = 4000.min(scale_1000 + 50);
                    sequencer.set_scaling_1000(scale_1000);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Minus),
                    ..
                } => {
                    scale_1000 = 250.max(scale_1000 - 50);
                    sequencer.set_scaling_1000(scale_1000);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Up),
                    ..
                } => {
                    let pos_us = pos_us + 5_000_000;
                    if paused {
                        sequencer.set_pos_us(pos_us);
                    }
                    else {
                        sequencer.play(pos_us, None, None);
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Down),
                    ..
                } => {
                    let pos_us = if pos_us > 5_000_000 {
                        pos_us - 5_000_000
                    }
                    else {
                        0
                    };
                    if paused {
                        sequencer.set_pos_us(pos_us);
                    }
                    else {
                        sequencer.play(pos_us, None, None);
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    shift_key += 1;
                    //opt_waterfall = None;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    shift_key -= 1;
                    //opt_waterfall = None;
                }
                Event::MultiGesture {
                    timestamp,
                    touch_id,
                    x,
                    y,
                    num_fingers,
                    ..
                } => {
                    //finger_msg = format!(
                    //    "t={} id={} fid={} x={:.2} y={:.2}",
                    //    timestamp, touch_id, num_fingers, x, y
                    //);
                }
                Event::FingerMotion {
                    timestamp: _timestamp,
                    touch_id: _touch_id,
                    finger_id: _finger_id,
                    x: _x,
                    y: _y,
                    dx: _dx,
                    dy: _dy,
                    pressure: _pressure,
                } => {
                    //finger_msg = format!("t={} id={} fid={} x={:.2} y={:.2} dx={:.2} dy={:.2}",
                    //                  timestamp, touch_id, finger_id,
                    //                  x,y,dx,dy);
                }
                _ => {}
            }
        }
        // The rest of the game loop goes here...
        trace!("{}",fps_manager.delay());
    }
    sleep(Duration::from_millis(150));
    Ok(())
}
