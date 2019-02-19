use font_kit;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::pixels::Color;
use log::*;

use piano_keyboard;

use crate::midi_sequencer;

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
                     pressed: bool)  -> Result<(), Box<std::error::Error>> {
    canvas.set_draw_color(sdl2::pixels::Color::RGB(100,100,100));
    canvas.clear();
    //let rec = canvas.viewport();
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
            canvas.fill_rect(rec)?;
        }
    }
    Ok(())
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

    let i = (i & 1) as u8 * 40;
    canvas.set_draw_color(sdl2::pixels::Color::RGB(140+i,140+i,140+i));
    canvas.clear();

    let left_key = keyboard.left_white_key;
    let mut rect_templates: Vec<sdl2::rect::Rect> = vec![];
    for el in keyboard.iter() {
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

    for (i,_el) in keyboard.iter().enumerate() {
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
        let y_shift = height as i64 - (tex_row_bottom-copy_row_top).min(height as i64);
        let y_dst = (tex_row_top - copy_row_top).max(0) as i32 + y_shift as i32; // Need to add something
        let y_src = overlap as i32 + net_rows as i32 - cp_height as i32
                            - (copy_row_top - tex_row_top).max(0) as i32;

        let src_rec = sdl2::rect::Rect::new(0,y_src,width,cp_height);
        let dst_rec = sdl2::rect::Rect::new(0,y_dst,width,cp_height);
        trace!("Copy {:?}->{:?}",src_rec,dst_rec);
        canvas.copy(&texture, src_rec, dst_rec)?;
    }
    Ok(())
}


