use std::io::{stdin, stdout, Write};
use std::thread::sleep;
use std::time::{Instant,Duration};

use log::*;
use simple_logging;

use midir::MidiOutput;
use midly;

use clap::{value_t, values_t};

use sdl2::keyboard::Keycode;
use sdl2::event::Event;

use crate::time_controller::TimeListenerTrait;

//mod app;
mod time_controller;
mod midi_container;
mod midi_sequencer;
mod draw_engine;
mod scroller;
mod usage;

fn transposed_message(time_us: u64,trk: usize,channel: u8,
                      message: &midly::MidiMessage,
                      all: bool,
                      shift_key: i8,
                      left_key: u8,
                      right_key: u8)
                -> Option<(u64,usize,midi_sequencer::MidiEvent)> {
    match (message,all) {
        (midly::MidiMessage::NoteOn(key, pressure),_) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            }
            else {
                Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOn(0*channel, 
                        shifted_key as u8,
                        pressure.as_int()),
                ))
            }
        },
        (midly::MidiMessage::NoteOff(key, pressure),_) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            }
            else {
                Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::NoteOff(0*channel, 
                        shifted_key as u8,
                        pressure.as_int()),
                ))
            }
        },
        (midly::MidiMessage::Aftertouch(key, pressure),true) => {
            let shifted_key = key.as_int() as i16 + shift_key as i16;
            if shifted_key < left_key as i16 || shifted_key > right_key as i16 {
                None
            }
            else {
                Some((
                    time_us,
                    trk,
                    midi_sequencer::MidiEvent::Aftertouch(0*channel, 
                        shifted_key as u8,
                        pressure.as_int()),
                ))
            }
        },
        (midly::MidiMessage::Controller(control, value),true) => Some((
            time_us,
            trk,
            midi_sequencer::MidiEvent::Controller(channel, control.as_int(), value.as_int()),
        )),
        (midly::MidiMessage::ChannelAftertouch(pressure),true) => Some((
            time_us,
            trk,
            midi_sequencer::MidiEvent::ChannelAftertouch(channel, pressure.as_int()),
        )),
        (midly::MidiMessage::PitchBend(change),true) => Some((
            time_us,
            trk,
            midi_sequencer::MidiEvent::PitchBend(channel, change.as_int()),
        )),
        (midly::MidiMessage::ProgramChange(program),true) => Some((
            time_us,
            trk,
            midi_sequencer::MidiEvent::ProgramChange(channel, program.as_int()),
        )),
        (_,false) => None,
    }
}

fn main() -> Result<(), Box<std::error::Error>> {
    let matches = usage::usage();

    let debug = matches.is_present("debug");
    simple_logging::log_to_stderr(if debug {
        LevelFilter::Trace
    } else {
        if matches.is_present("verbose") {
            LevelFilter::Warn
        }
        else {
            LevelFilter::Error
        }
    });

    let mut shift_key = value_t!(matches, "transpose", i8).unwrap_or_else(|e| e.exit());

    // MIDI notes are numbered from 0 to 127 assigned to C-1 to G9
    let rd64 = matches.is_present("RD64");
    let (left_key, right_key): (u8,u8) = if rd64 {
        // RD-64 is A1 to C7
        (21 + 12, 108 - 12)
    } else {
        // 88 note piano range from A0 to C8
        (21, 108)
    };

    let nr_of_keys = right_key - left_key + 1;

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
    let mut show_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
        .filter_map(|(time_us, trk, evt)| match evt {
            midly::EventKind::Midi { channel, message } => 
                transposed_message(time_us,trk,channel.as_int(),message,false,shift_key,left_key,right_key),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut play_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
        .filter_map(|(time_us, trk, evt)| match evt {
            midly::EventKind::Midi { channel, message } => 
                transposed_message(time_us,trk,channel.as_int(),message,true,shift_key,left_key,right_key),
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
    println!("display driver: {:?}",video_subsystem.current_video_driver());
    println!("dpi: {:?}",video_subsystem.display_dpi(0));
    println!("Screensaver: {:?}",video_subsystem.is_screen_saver_enabled());

    println!("Swap interval: {:?}",video_subsystem.gl_get_swap_interval());
    println!("{:?}",video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync));
    println!("Swap interval: {:?}",video_subsystem.gl_get_swap_interval());

    let window = video_subsystem
        .window(&format!("Rusthesia: {}", midi_fname), 800, 600)
        .position_centered()
        .resizable()
        .build()
        .unwrap();
    println!("Display Mode: {:?}",window.display_mode());
    //let window_context = window.context();
    let mut canvas = sdl2::render::CanvasBuilder::new(window)
        .accelerated()
        .build()?;
    let texture_creator = canvas.texture_creator();
    let mut textures: Vec<sdl2::render::Texture> = vec![];

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut paused = false;
    let mut scale_1000 = 1000;
    let mut opt_keyboard: Option<piano_keyboard::Keyboard2d> = None;
    let rows_per_s = 100;
    let waterfall_tex_height = 1000;

    let base_time = Instant::now();
    let ms_per_frame = 40;
    let mut frame_cnt = 0;
    let mut scroll = scroller::Scroller::new(5_000_000.0);

    let time_keeper = sequencer.get_new_listener();
    sequencer.play(-3_000_000, Some(scale_1000), None);
    'running: loop {
        let rec = canvas.viewport();
        let width = rec.width();
        let waterfall_overlap = 2*width/nr_of_keys as u32; // ensure even
        let waterfall_net_height = waterfall_tex_height - waterfall_overlap;

        if opt_keyboard.is_some() {
            if opt_keyboard.as_ref().unwrap().width != width as u16 {
                opt_keyboard = None;
            }
        }
        if opt_keyboard.is_none() {
            trace!("Create Keyboard");
            textures.clear();
            opt_keyboard = Some(piano_keyboard::KeyboardBuilder::new()
                                .set_width(rec.width() as u16)?
                                .white_black_gap_present(true)
                                .set_most_left_right_white_keys(left_key, right_key)?
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
                canvas.with_texture_canvas(&mut texture, |tex_canvas| {
                    draw_engine::draw_keyboard(keyboard,tex_canvas,pressed).ok();
                })?;
                textures.push(texture);
            }

            // Texture 2.. are for waterfall.
            //
            let maxtime_us = show_events[show_events.len()-1].0;
            let rows = (maxtime_us * rows_per_s as u64+ 999_999) / 1_000_000;
            let nr_of_textures = ((rows + waterfall_net_height as u64 - 1)
                                    /waterfall_net_height as u64) as u32;
            trace!("Needed rows/textures: {}/{}",rows,nr_of_textures);
            for i in 0..nr_of_textures {
                let mut texture = texture_creator
                    .create_texture_target(texture_creator.default_pixel_format(),
                                            width, waterfall_tex_height)
                    .unwrap();
                canvas.with_texture_canvas(&mut texture, |tex_canvas| {
                    draw_engine::draw_waterfall(keyboard,tex_canvas,i,
                                i*waterfall_net_height, waterfall_net_height,
                                waterfall_overlap,
                                rows_per_s,&show_events);
                })?;
                textures.push(texture);
            }
            
        }


        let elapsed = base_time.elapsed();
        let elapsed_us = elapsed.subsec_micros();
        let rem_us = ms_per_frame * 1_000 - elapsed_us % (ms_per_frame * 1_000);
        let rem_dur =  Duration::new(0, rem_us*1_000);
        let pos_us: i64 = time_keeper.get_pos_us_after(rem_dur);
        trace!("pos_us={}",pos_us);

        // Clear canvas
        canvas.set_draw_color(sdl2::pixels::Color::RGB(50,50,50));
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

        trace!("before draw_engine::copy_waterfall_to_screen");
        if true {
            draw_engine::copy_waterfall_to_screen(
                &textures[2..],
                &mut canvas,
                rec.width(),
                rec.height()-keyboard.height as u32,
                waterfall_net_height,
                waterfall_overlap,
                rows_per_s,
                pos_us)?;
        }

        trace!("before Eventloop");
        //for event in event_pump.poll_iter() {
        let mut empty = false;
        let sleep_duration = loop {
            let elapsed = base_time.elapsed();
            let elapsed_us = elapsed.subsec_micros() as u64
                                + elapsed.as_secs()*1_000_000;
            let next_frame_cnt = elapsed_us / (ms_per_frame as u64 * 1_000) + 1;
            if next_frame_cnt > frame_cnt+1 {
                warn!("FRAME LOST curr={} next={}", frame_cnt, next_frame_cnt);
            }
            frame_cnt = next_frame_cnt;
            let rem_us = ms_per_frame as u64* 1_000 - elapsed_us % (ms_per_frame as u64* 1_000);
            if empty || rem_us < 5_000 {
                break Duration::new(0, rem_us as u32*1_000);
            }

            if let Some(event) = event_pump.wait_event_timeout(1) {
                trace!("event received: {:?}",event);
                match event {
                    Event::Window { win_event,.. } => {
                        trace!("Window Event: {:?}",win_event);
                    }
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
                        shift_key -= 1;
                        sequencer.stop();
                        show_events = container
                            .iter()
                            .timed(&container.header().timing)
                            .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
                            .filter_map(|(time_us, trk, evt)| match evt {
                                midly::EventKind::Midi { channel, message } => 
                                    transposed_message(time_us,trk,channel.as_int(),message,false,shift_key,left_key,right_key),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        play_events = container
                            .iter()
                            .timed(&container.header().timing)
                            .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
                            .filter_map(|(time_us, trk, evt)| match evt {
                                midly::EventKind::Midi { channel, message } => 
                                    transposed_message(time_us,trk,channel.as_int(),message,true,shift_key,left_key,right_key),
                                _ => None,
                            })
                            .inspect(|e| trace!("{:?}",e))
                            .collect::<Vec<_>>();
                        textures.clear();
                        sequencer.play(pos_us, None, Some(play_events));
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Right),
                        ..
                    } => {
                        shift_key += 1;
                        sequencer.stop();
                        show_events = container
                            .iter()
                            .timed(&container.header().timing)
                            .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
                            .filter_map(|(time_us, trk, evt)| match evt {
                                midly::EventKind::Midi { channel, message } => 
                                    transposed_message(time_us,trk,channel.as_int(),message,false,shift_key,left_key,right_key),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        play_events = container
                            .iter()
                            .timed(&container.header().timing)
                            .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
                            .filter_map(|(time_us, trk, evt)| match evt {
                                midly::EventKind::Midi { channel, message } => 
                                    transposed_message(time_us,trk,channel.as_int(),message,true,shift_key,left_key,right_key),
                                _ => None,
                            })
                            .inspect(|e| trace!("{:?}",e))
                            .collect::<Vec<_>>();
                        textures.clear();
                        sequencer.play(pos_us, None, Some(play_events));
                    }
                    Event::MultiGesture {
                        timestamp: _timestamp,
                        touch_id: _touch_id,
                        x: _x,
                        y,
                        num_fingers,
                        ..
                    } => {
                        //finger_msg = format!(
                        //    "t={} id={} fid={} x={:.2} y={:.2}",
                        //    timestamp, touch_id, num_fingers, x, y
                        //);
                        trace!("Finger {} {}",y, num_fingers);
                        if num_fingers == 2 {
                            if !scroll.update_move(y, ms_per_frame) {
                                sequencer.stop();
                            }
                        }
                    }
                    Event::FingerDown {
                        timestamp: _timestamp,
                        touch_id: _touch_id,
                        finger_id: _finger_id,
                        x: _x,
                        y: _y,
                        dx: _dx,
                        dy: _dy,
                        pressure: _pressure,
                    } => {
                        if scroll.stop() && !paused {
                            sequencer.play(pos_us,None,None);
                        }
                    }
                    Event::FingerUp {
                        timestamp: _timestamp,
                        touch_id: _touch_id,
                        finger_id: _finger_id,
                        x: _x,
                        y: _y,
                        dx: _dx,
                        dy: _dy,
                        pressure: _pressure,
                    } => {
                        scroll.end_move();
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
            else {
                empty = true;
            }
        };

        if let Some((is_end,delta)) = scroll.update_position(ms_per_frame) {
            if is_end && !paused {
                sequencer.play(pos_us + delta as i64,None,None);
            }
            else {
                sequencer.set_pos_us(pos_us + delta as i64);
            }
        }

        trace!("before sleep {:?}",sleep_duration);
        std::thread::sleep(sleep_duration);

        trace!("before canvas present");
        canvas.present();
    }
    sleep(Duration::from_millis(150));
    Ok(())
}
