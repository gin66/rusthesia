use std::time::Instant;

use font_kit;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::pixels::Color;
use log::*;

use piano_keyboard;

use crate::midi_sequencer;

#[derive(Copy, Clone)]
enum NoteState {
    Pressed(usize),
    Keep(usize),
    Off,
}

fn trk2col(trk: usize, key: u32) -> Color {
    match (trk % 2, true) { //is_white(key)) {
        (0, true) => Color::RGB(0, 255, 255),
        (0, false) => Color::RGB(0, 200, 200),
        (_, true) => Color::RGB(255, 0, 255),
        (_, false) => Color::RGB(200, 0, 200),
    }
}

pub fn draw_keyboard(keyboard: &piano_keyboard::Keyboard2d,
                     canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
                     pressed: bool) {
    canvas.set_draw_color(sdl2::pixels::Color::RGB(100,100,100));
    canvas.clear();
    let rec = canvas.viewport();
    let (col_white,col_black) = if pressed  {
        (Color::RGB(100, 255, 255),Color::RGB( 50, 150, 150))
    }
    else {
        (Color::RGB(200, 200, 200),Color::RGB(  0,   0,   0))
    };
    
    for (col,rects) in vec![(col_white,keyboard.white_keys(true)),
                            (col_black,keyboard.black_keys())].drain(..) {
        canvas.set_draw_color(col);
        for rect in rects.into_iter() {
            let rec = sdl2::rect::Rect::new(
                rect.x as i32,
                rect.y as i32,
                rect.width as u32,
                rect.height as u32);
            canvas.fill_rect(rec);
        }
    }
}

pub fn get_pressed_key_rectangles(keyboard: &piano_keyboard::Keyboard2d,
                               height_offset: u32,
                               pos_us: i64,
                               show_events: &Vec<(u64, usize,
                                            midi_sequencer::MidiEvent)>)
                            -> Vec<(sdl2::rect::Rect,sdl2::rect::Rect)> {
    let nr_of_keys = keyboard.right_white_key-keyboard.left_white_key+1;
    let mut pressed = vec![0; nr_of_keys as usize];
    let left_key = keyboard.left_white_key;

    for (time, _, evt) in show_events.iter() {
        // TODO: This needs more work. Adjacent midi notes are shown continuously...
        if (*time as i64) > pos_us {
            break;
        }
        if (*time as i64) + 50_000 > pos_us {
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure) =>
                    pressed[(key-left_key) as usize] = *pressure,
                _ => ()
            }
        }
        else {
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure) =>
                    pressed[(key-left_key) as usize] = *pressure,
                midi_sequencer::MidiEvent::NoteOff(_channel, key, _) =>
                    pressed[(key-left_key) as usize] = 0,
                _ => ()
            }
        }
    }

    let mut highlight = vec![];
    for (el,is_pressed) in keyboard.iter().zip(pressed.iter()) {
        if *is_pressed > 0 {
            let rects = match *el {
                piano_keyboard::Element::WhiteKey {
                    wide: ref r1, small: ref r2, blind: Some(ref r3) } => vec![r1,r2,r3],
                piano_keyboard::Element::WhiteKey { 
                    wide: ref r1, small: ref r2, blind: None } => vec![r1,r2],
                piano_keyboard::Element::BlackKey(ref r1) => vec![r1]
            };
            for r in rects.into_iter() {
                let src_rec = sdl2::rect::Rect::new(r.x as i32, r.y as i32,
                                                    r.width as u32,r.height as u32);
                let dst_rec = sdl2::rect::Rect::new(r.x as i32,
                                                    (r.y as u32 + height_offset) as i32,
                                                    r.width as u32,r.height as u32);
                highlight.push( (src_rec,dst_rec) );
            }
        }
    }
    highlight
}

pub fn draw_waterfall(keyboard: &piano_keyboard::Keyboard2d,
                      canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
                      i: u32,
                      bottom_row: u32,
                      net_rows: u32,
                      overlap: u32,
                      rows_per_s: u32,
                      show_events: &Vec<(u64, usize,
                                            midi_sequencer::MidiEvent)>) {
    // The waterfall is flowing from top to bottom with SDL having origin top left.
    // Thus every texture has to fill from bottom to top.

    let i = i as u8 * 40;
    canvas.set_draw_color(sdl2::pixels::Color::RGB(140+i,140+i,140+i));
    canvas.clear();

    let left_key = keyboard.left_white_key;
    let mut rect_templates: Vec<sdl2::rect::Rect> = vec![];
    for (i,el) in keyboard.iter().enumerate() {
        let (x,width) = match *el {
            piano_keyboard::Element::WhiteKey {
                wide: _, small: ref r1, blind: Some(ref r2) }
                                => (r1.x.min(r2.x),r1.width+r2.width),
            piano_keyboard::Element::WhiteKey { 
                wide: _, small: ref r, blind: None }
            | piano_keyboard::Element::BlackKey(ref r) => (r.x,r.width)
        };
        rect_templates.push(sdl2::rect::Rect::new(x as i32, 0, width as u32, 0));
    }

    for (i,el) in keyboard.iter().enumerate() {
        let sel_key = left_key + i as u8;
        let mut opt_start = None;
        let mut opt_end = None;
        for (time, _, evt) in show_events.iter() {
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure)
                            if *key == sel_key && *pressure > 0 => {
                    opt_start = Some((time * rows_per_s as u64/ 1_000_000) as u32);
                    trace!("{}: {:?}  {:?}",time,evt,opt_start);
                },
                midi_sequencer::MidiEvent::NoteOn(_channel, key, 0) 
                | midi_sequencer::MidiEvent::NoteOff(_channel, key, _)
                            if *key == sel_key => {
                    opt_end = Some((time * rows_per_s as u64/ 1_000_000) as u32);
                    trace!("{}: {:?}  {:?}",time,evt,opt_end);
                }
                _ => continue
            }
            match (opt_start,opt_end) {
                (Some(start),Some(end)) => {
                    if start > bottom_row+net_rows+overlap {
                        continue;
                    }
                    if end <= bottom_row {
                        continue;
                    }
                    trace!("start/end = {}/{}",start,end);
                    let start_row = start.max(bottom_row);
                    let end_row = end.min(bottom_row+net_rows+overlap-1);
                    trace!("{} {}",start_row,end_row);
                    let height = end_row - start_row + 1;
                    let tex_y = bottom_row+net_rows+overlap-1 - end_row; // flip

                    let mut rec = rect_templates[i].clone();
                    rec.set_y(tex_y as i32);
                    rec.set_height(height);
                    trace!("Need draw: {:?}",rec);
                    let rounding = rec.width() as i16/ 2 - 1;
                    // later change to draw two circles and a rectangle
                    canvas.rounded_box(
                        rec.left() as i16,
                        rec.bottom() as i16 - rounding as i16/2+1,
                        rec.right() as i16,
                        rec.top() as i16 + rounding as i16/2-1,
                        rounding,
                        Color::RGB(255,255,255)
                        ).unwrap();
                    opt_start = None;
                    opt_end = None;
                },
                (None,Some(_)) => {
                    opt_end = None;
                    warn!("Note Off with Note On should not happen")
                },
                _ => ()
            }
        }
    }
}


pub fn copy_waterfall_to_screen(
                      textures: &[sdl2::render::Texture],
                      canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
                      width: u32,
                      height: u32,
                      net_rows: u32,
                      overlap: u32,
                      rows_per_s: u32,
                          pos_us: i64
    )  -> Result<(), Box<std::error::Error>> {
    // if pos_us = 0, then first texture bottom need to reach keyboard
    // Thus 
    //      src_rect.x=net_rows+overlap-height,src_rect.height=height
    //      dst_rect.x=0,dst_rect.height=height

    // rows to display
    //    top/bottom as on display visible, with y(top) < y(bottom)
    let row_top = pos_us * rows_per_s as i64 / 1_000_000;
    let row_bottom = row_top + height as i64;

    for (i,ref texture) in textures.iter().enumerate() {
        let tex_row_top = (i as u32 * net_rows) as i64;
        let tex_row_bottom = tex_row_top + net_rows as i64 - 1;

        let copy_row_top = row_top.max(tex_row_top);
        let copy_row_bottom = row_bottom.min(tex_row_bottom);
        trace!("Texture {}: Overlap texture {}-{}<>requested area {}-{} => {}-{}",
               i,
               tex_row_top, tex_row_bottom,
               row_top, row_bottom,
               copy_row_top, copy_row_bottom);

        if copy_row_top > copy_row_bottom {
            continue;
        }
        // There is an overlap of texture and canvas.

        let cp_height = (copy_row_bottom - copy_row_top + 1) as u32;
        let y_dst = (tex_row_top - copy_row_top).max(0) as i32;
        let y_src = overlap as i32 + net_rows as i32 - cp_height as i32
                            - (copy_row_top - tex_row_top).max(0) as i32;

        let src_rec = sdl2::rect::Rect::new(0,y_src,width,cp_height);
        let dst_rec = sdl2::rect::Rect::new(0,y_dst,width,cp_height);
        println!("Copy {:?}->{:?}",src_rec,dst_rec);
        canvas.copy(&texture, src_rec, dst_rec)?;
    }
    Ok(())
}



pub struct DrawEngine {
}
impl DrawEngine {
    pub fn init(video_subsystem: sdl2::VideoSubsystem) -> Result<DrawEngine, Box<std::error::Error>> {
        let midi_fname = format!("fname");
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

        Ok(DrawEngine {
        })
        //Err(Error::new(ErrorKind::Other, "oh no!"))
    }

    pub fn draw(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
                textures: &mut Vec<sdl2::render::Texture>,
                pos_us: i64) -> Result<(), Box<std::error::Error>> {
        let mut paused = false;
        let mut pos_us = 0;
        let mut scale_1000 = 1000;
        let mut finger_msg = format!("----");
        let maxtime = 100;
        let opt_font: Option<sdl2::ttf::Font> = None;
        let (left_key,right_key) = (21,108);

        canvas.set_draw_color(Color::RGB(0, 255, 255));
        canvas.clear();
        canvas.present();

        let mut opt_last_draw_instant: Option<Instant> = None;
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

            let left_white_key = 0;//key_to_white(left_key);
            let right_white_key = 0;//key_to_white(right_key);
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
                        let nx = 0;//key_to_white(key);
                        let r = sdl2::rect::Rect::new(
                            offset_x + (nx * white_key_width + nx * white_key_space) as i32,
                            rec.bottom() - white_key_height as i32,
                            white_key_width,
                            white_key_height,
                        );
                        traces.push(r.clone());
                        //match timeline[curr_pos].2[key as i8 as usize] {
                        //    NoteState::Pressed(_) | NoteState::Keep(_) => white_keys_on.push(r),
                        //    NoteState::Off => white_keys.push(r),
                        //}
                    }
                    1 | 3 | 6 | 8 | 10 => {
                        // black keys
                        let nx = 0;//key_to_white(key);
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
                        //match timeline[curr_pos].2[key as i8 as usize] {
                            //NoteState::Pressed(_) | NoteState::Keep(_) => black_keys_on.push(r),
                            //NoteState::Off => black_keys.push(r),
                        //}
                    }
                    _ => (),
                }
            }

            let mut opt_waterfall: Option<sdl2::render::Texture> = None;
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
                let mut wf_canvas: sdl2::render::Canvas<sdl2::surface::Surface> = sf.into_canvas()?;

                wf_canvas.set_draw_color(Color::RGB(100, 100, 100));
                wf_canvas.clear();

                for key in left_key..=right_key {
                    let mut last_y = height;
                    let mut t_rect = traces.remove(0);
                    let mut state = NoteState::Off;
                    //for p in 0..timeline.len() {
                    //    let p_t = timeline[p].0.min(maxtime);
                    //    let new_y = (p_t as u64 * height as u64 / maxtime as u64) as u32;
                    //    let new_y = height - new_y;
                    //    let new_state = timeline[p].2[key as i8 as usize];
                    //    match (state, new_state) {
                    //        (NoteState::Pressed(_), NoteState::Keep(_)) => (),
                    //        (NoteState::Pressed(trk), NoteState::Off)
                    //        | (NoteState::Keep(trk), NoteState::Off) => {
                    //            t_rect.set_height((last_y - new_y) as u32);
                    //            t_rect.set_bottom(last_y as i32);
                    //            wf_canvas.set_draw_color(Color::RGB(0, 255, 255));
                    //            wf_canvas
                    //                .rounded_box(
                    //                    t_rect.left() as i16,
                    //                    t_rect.bottom() as i16,
                    //                    t_rect.right() as i16,
                    //                    t_rect.top() as i16,
                    //                    box_rounding,
                    //                    trk2col(trk, key),
                    //                )
                    //                .unwrap();
                    //            last_y = new_y;
                    //        }
                    //        (NoteState::Pressed(_), NoteState::Pressed(trk))
                    //        | (NoteState::Keep(_), NoteState::Pressed(trk)) => {
                    //            t_rect.set_height((last_y - new_y - 2) as u32);
                    //            t_rect.set_bottom(last_y as i32);
                    //            wf_canvas.set_draw_color(Color::RGB(0, 255, 255));
                    //            wf_canvas
                    //                .rounded_box(
                    //                    t_rect.left() as i16,
                    //                    t_rect.bottom() as i16,
                    //                    t_rect.right() as i16,
                    //                    t_rect.top() as i16,
                    //                    box_rounding,
                    //                    trk2col(trk, key),
                    //                )
                    //                .unwrap();
                    //            last_y = new_y;
                    //        }
                    //        (NoteState::Keep(_), NoteState::Keep(_)) => (),
                    //        (NoteState::Off, NoteState::Keep(_))
                    //        | (NoteState::Off, NoteState::Pressed(_))
                    //        | (NoteState::Off, NoteState::Off) => {
                    //            last_y = new_y;
                    //        }
                    //    };
                    //    state = new_state;
                    //}
                }
                //let surface: sdl2::surface::Surface = wf_canvas.into_surface();
                //let texture: sdl2::render::Texture = 
                //    texture_creator.create_texture_from_surface(surface)?;
                //textures.push(texture);
            }

            if false {
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
            }
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
                //lines.push(format!("shift = {}", shift_key));
                lines.push(finger_msg.clone());

                let mut y = 10;
                for line in lines.into_iter() {
                    if let Ok((width, height)) = font.size_of(&line) {
                        if let Ok(surface) =
                            font.render(&line).solid(Color::RGBA(255, 255, 255, 255))
                        {
                            //let demo_tex = texture_creator
                            //    .create_texture_from_surface(surface)
                            //    .unwrap();
                            //canvas
                            //    .copy(&demo_tex, None, sdl2::rect::Rect::new(10, y, width, height))
                            //    .unwrap();
                            y += height as i32 + 2;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
