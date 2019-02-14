use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use log::*;
use simple_logging;

use midir::MidiOutput;
use midly;

use clap::{value_t, values_t};

use font_kit;
use sdl2::event::Event;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;

mod time_controller;
mod midi_container;
mod midi_sequencer;
mod keyboard;
mod usage;

#[derive(Copy, Clone)]
enum NoteState {
    Pressed(usize),
    Keep(usize),
    Off,
}

// http://www.rwgiangiulio.com/construction/manual/layout.jpg
fn is_white(key: u32) -> bool {
    match key % 12 {
        0 | 2 | 4 | 5 | 7 | 9 | 11 => true,
        1 | 3 | 6 | 8 | 10 => false,
        _ => panic!("wrong value"),
    }
}

fn key_to_white(key: u32) -> u32 {
    match key % 12 {
        n @ 0 | n @ 2 | n @ 4 | n @ 5 | n @ 7 | n @ 9 | n @ 11 => (n + 1) / 2 + (key / 12) * 7,
        n @ 1 | n @ 3 | n @ 6 | n @ 8 | n @ 10 => n / 2 + (key / 12) * 7,
        _ => panic!("wrong value"),
    }
}

fn trk2col(trk: usize, key: u32) -> Color {
    match (trk % 2, is_white(key)) {
        (0, true) => Color::RGB(0, 255, 255),
        (0, false) => Color::RGB(0, 200, 200),
        (_, true) => Color::RGB(255, 0, 255),
        (_, false) => Color::RGB(200, 0, 200),
    }
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

    // old code
    let midi_fname = matches.value_of("MIDI").unwrap();
    let mut f = File::open(midi_fname)?;
    let mut midi = Vec::new();
    f.read_to_end(&mut midi)?;
    let smf: midly::Smf<Vec<midly::Event>> = midly::Smf::read(&midi)?;

    // new code using midi_container
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

    // old code
    let ppqn = match container.header().timing {
        midly::Timing::Metrical(x) => x.as_int() as u32,
        midly::Timing::Timecode(_x, _y) => panic!("Timecode not implemented"),
        //  https://en.wikipedia.org/wiki/MIDI_timecode
    };

    // new code to be refactored in separate function
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
    let _show_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| show_tracks.contains(trk))
        .collect::<Vec<_>>();
    let play_events = container
        .iter()
        .timed(&container.header().timing)
        .filter(|(_time_us, trk, _evt)| play_tracks.contains(trk))
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
            },
            _ => None,
        })
        .inspect(|e| trace!("{:?}",e))
        .collect::<Vec<_>>();

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

    // Reorder all midi message on timeline
    let mut tracks = vec![];
    for i in 0..smf.tracks.len() {
        let st = show_tracks.contains(&i);
        let pt = play_tracks.contains(&i);
        tracks.push((0, 0, i, smf.tracks[i].iter(), None, st, pt));
    }

    let mut timeline = vec![(0, vec![], vec![NoteState::Off; 128])];
    let mut microseconds_per_beat = None;
    let mut maxtime = 0;
    loop {
        if tracks.len() > 1 {
            tracks.sort_by_key(|x| {
                u32::max_value()
                    - x.0
                    - match (x.1 as u32, microseconds_per_beat) {
                        (0, _) => 0,
                        (ticks, None) => ticks,
                        (ticks, Some(mspb)) => {
                            (ticks as u64 * mspb as u64 / ppqn as u64 / 1000) as u32
                        }
                    }
            });
        }
        if let Some((t, ticks, i, mut t_iter, m, st, pt)) = tracks.pop() {
            let dt = match ticks as u64 {
                0 => 0,
                ticks => {
                    (ticks * microseconds_per_beat.unwrap() as u64 / ppqn as u64 / 1000) as u32
                }
            };
            let t = t + dt;
            let mut n = timeline.len() - 1;
            if t > timeline[n].0 {
                let note_state = timeline[timeline.len() - 1]
                    .2
                    .iter()
                    .map(|ns| match ns {
                        NoteState::Pressed(trk) | NoteState::Keep(trk) => NoteState::Keep(*trk),
                        NoteState::Off => NoteState::Off,
                    })
                    .collect::<Vec<_>>();
                timeline.push((t, vec![], note_state));
                n += 1;
            }
            if pt {
                timeline[n].1.push(m.clone());
            }
            if st {
                match m {
                    Some((_c, midly::MidiMessage::NoteOn(p1, p2))) => {
                        timeline[n].2[p1.as_int() as usize] = if p2.as_int() > 0 {
                            NoteState::Pressed(i)
                        } else {
                            NoteState::Off
                        };
                        maxtime = timeline[timeline.len() - 1].0;
                    }
                    Some((_c, midly::MidiMessage::NoteOff(p1, _p2))) => {
                        timeline[n].2[p1.as_int() as usize] = NoteState::Off;
                        maxtime = timeline[timeline.len() - 1].0;
                    }
                    v => println!("=> {:#?}", v),
                }
            }
            if let Some(ev) = t_iter.next() {
                match ev.kind {
                    midly::EventKind::Meta(midly::MetaMessage::Tempo(ms_per_beat)) => {
                        println!("Tempo => {:#?}", ev);
                        microseconds_per_beat = Some(ms_per_beat.as_int());
                    }
                    _ => (),
                }
                match ev.kind {
                    midly::EventKind::Midi {
                        channel: c,
                        message: m,
                    } => {
                        tracks.push((t, ev.delta.as_int(), i, t_iter, Some((c, m)), st, pt));
                    }
                    _ => {
                        println!("=> {:#?}", ev);
                        tracks.push((t, ev.delta.as_int(), i, t_iter, None, st, pt));
                    }
                }
            }
        } else {
            break;
        }
    }

    //return Ok(());

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let ttf_context = sdl2::ttf::init().unwrap();
    let opt_font = if let Ok(font) =
        font_kit::source::SystemSource::new().select_by_postscript_name("ArialMT")
    {
        let res_font = match font {
            font_kit::handle::Handle::Path { path, font_index } => {
                ttf_context.load_font_at_index(path, font_index, 24)
            }
            font_kit::handle::Handle::Memory {
                bytes: _bytes,
                font_index: _font_index,
            } => {
                //let bytes = (*bytes).clone();
                //let buf = sdl2::rwops::RWops::from_read(bytes).unwrap();
                //ttf_context.load_font_at_index_from_rwops(buf,font_index,24)
                Err("not supported".to_string())
            }
        };
        res_font.ok()
    } else {
        None
    };
    println!("Have font={:?}", opt_font.is_some());

    let window = video_subsystem
        .window(&format!("Rusthesia: {}", midi_fname), 800, 600)
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

    let curr_pos = 0;
    let mut paused = false;
    let mut opt_waterfall: Option<sdl2::render::Texture> = None;
    let mut opt_last_draw_instant: Option<Instant> = None;
    let mut finger_msg = format!("----");
    let mut scale_1000 = 1000;
    sequencer.play(0, Some(scale_1000), None);
    'running: loop {
        let pos_us: i64 = sequencer.pos_us();
        if opt_last_draw_instant
            .map(|x| x.elapsed().subsec_millis() > 20)
            .unwrap_or(true)
        {
            opt_last_draw_instant = Some(Instant::now());
            canvas.set_draw_color(Color::RGB(50, 50, 50));
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
                            NoteState::Pressed(_) | NoteState::Keep(_) => white_keys_on.push(r),
                            NoteState::Off => white_keys.push(r),
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
                            NoteState::Pressed(_) | NoteState::Keep(_) => black_keys_on.push(r),
                            NoteState::Off => black_keys.push(r),
                        }
                    }
                    _ => (),
                }
            }

            if opt_waterfall.is_some() {
                if opt_waterfall.as_ref().unwrap().query().width != rec.width() {
                    opt_waterfall = None;
                }
            }
            if opt_waterfall.is_none() {
                let width = rec.width();
                let height = (rec.height() * maxtime / 5_000).min(16384);
                println!(
                    "Waterfall size: {}x{}   maxtime = {}  height={}",
                    width,
                    height,
                    maxtime,
                    rec.height()
                );
                let sf = sdl2::surface::Surface::new(
                    width,
                    height,
                    sdl2::pixels::PixelFormatEnum::RGB888,
                )?;
                let mut wf_canvas = sf.into_canvas()?;

                wf_canvas.set_draw_color(Color::RGB(100, 100, 100));
                wf_canvas.clear();

                for key in left_key..=right_key {
                    let mut last_y = height;
                    let mut t_rect = traces.remove(0);
                    let mut state = NoteState::Off;
                    for p in 0..timeline.len() {
                        let p_t = timeline[p].0.min(maxtime);
                        let new_y = (p_t as u64 * height as u64 / maxtime as u64) as u32;
                        let new_y = height - new_y;
                        let new_state = timeline[p].2[(key as i8 + shift_key) as usize];
                        match (state, new_state) {
                            (NoteState::Pressed(_), NoteState::Keep(_)) => (),
                            (NoteState::Pressed(trk), NoteState::Off)
                            | (NoteState::Keep(trk), NoteState::Off) => {
                                t_rect.set_height((last_y - new_y) as u32);
                                t_rect.set_bottom(last_y as i32);
                                wf_canvas.set_draw_color(Color::RGB(0, 255, 255));
                                wf_canvas
                                    .rounded_box(
                                        t_rect.left() as i16,
                                        t_rect.bottom() as i16,
                                        t_rect.right() as i16,
                                        t_rect.top() as i16,
                                        box_rounding,
                                        trk2col(trk, key),
                                    )
                                    .unwrap();
                                last_y = new_y;
                            }
                            (NoteState::Pressed(_), NoteState::Pressed(trk))
                            | (NoteState::Keep(_), NoteState::Pressed(trk)) => {
                                t_rect.set_height((last_y - new_y - 2) as u32);
                                t_rect.set_bottom(last_y as i32);
                                wf_canvas.set_draw_color(Color::RGB(0, 255, 255));
                                wf_canvas
                                    .rounded_box(
                                        t_rect.left() as i16,
                                        t_rect.bottom() as i16,
                                        t_rect.right() as i16,
                                        t_rect.top() as i16,
                                        box_rounding,
                                        trk2col(trk, key),
                                    )
                                    .unwrap();
                                last_y = new_y;
                            }
                            (NoteState::Keep(_), NoteState::Keep(_)) => (),
                            (NoteState::Off, NoteState::Keep(_))
                            | (NoteState::Off, NoteState::Pressed(_))
                            | (NoteState::Off, NoteState::Off) => {
                                last_y = new_y;
                            }
                        };
                        state = new_state;
                    }
                }
                opt_waterfall =
                    Some(texture_creator.create_texture_from_surface(wf_canvas.into_surface())?);
            }


            let wf_win_height = (rec.bottom() - white_key_height as i32) as u32;

            let wf_height = opt_waterfall.as_ref().unwrap().query().height;
            let y_shift =
                (pos_us as u64/1_000 * wf_height as u64 / maxtime as u64) as u32 + wf_win_height;
            let (y_src, y_dst, y_height) = if y_shift > wf_height {
                let dy = y_shift - wf_height;
                if wf_win_height >= dy {
                    (0, dy, wf_win_height - dy)
                }
                else {
                    (0, dy, 1)
                }
            } else {
                (wf_height - y_shift.min(wf_height), 0, wf_win_height)
            };
            let src_rect = sdl2::rect::Rect::new(0, y_src as i32, rec.width(), y_height);
            let dst_rect = sdl2::rect::Rect::new(0, y_dst as i32, rec.width(), y_height);
            canvas.copy(opt_waterfall.as_ref().unwrap(), src_rect, dst_rect)?;

            canvas.set_draw_color(Color::RGB(200, 200, 200));
            canvas.fill_rects(&white_keys).unwrap();
            canvas.set_draw_color(Color::RGB(255, 255, 255));
            canvas.fill_rects(&white_keys_on).unwrap();

            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.fill_rects(&black_keys).unwrap();
            canvas.set_draw_color(Color::RGB(0, 0, 255));
            canvas.fill_rects(&black_keys_on).unwrap();

            if let Some(ref font) = opt_font.as_ref() {
                let mut lines = vec![];
                lines.push(format!("{} ms", pos_us/1_000));
                lines.push(format!("scale = {:.2}",scale_1000 as f32/1000.0));
                lines.push(format!("shift = {}", shift_key));
                lines.push(finger_msg.clone());

                let mut y = 10;
                for line in lines.into_iter() {
                    if let Ok((width, height)) = font.size_of(&line) {
                        if let Ok(surface) =
                            font.render(&line).solid(Color::RGBA(255, 255, 255, 255))
                        {
                            let demo_tex = texture_creator
                                .create_texture_from_surface(surface)
                                .unwrap();
                            canvas
                                .copy(&demo_tex, None, sdl2::rect::Rect::new(10, y, width, height))
                                .unwrap();
                            y += height as i32 + 2;
                        }
                    }
                }
            }

            canvas.present();
        }

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
                    opt_waterfall = None;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    shift_key -= 1;
                    opt_waterfall = None;
                }
                Event::MultiGesture {
                    timestamp,
                    touch_id,
                    x,
                    y,
                    num_fingers,
                    ..
                } => {
                    finger_msg = format!(
                        "t={} id={} fid={} x={:.2} y={:.2}",
                        timestamp, touch_id, num_fingers, x, y
                    );
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
        sleep(Duration::from_millis(20));
    }
    sleep(Duration::from_millis(150));
    Ok(())
}
