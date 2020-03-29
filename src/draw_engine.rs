//use font_kit;
use log::*;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::pixels::Color;

use piano_keyboard;

use crate::midi_sequencer;

#[derive(Debug, PartialEq)]
pub enum DrawCommand {
    CopyToScreen {
        src_texture: usize,
        src_rect: sdl2::rect::Rect,
        dst_rect: sdl2::rect::Rect,
    },
}

fn is_white(key: u8) -> bool {
    match key % 12 {
        0 => true,
        1 => false,
        2 => true,
        3 => false,
        4 => true,
        5 => true,
        6 => false,
        7 => true,
        8 => false,
        9 => true,
        10 => false,
        11 => true,
        _ => panic!("Cannot happen"),
    }
}

fn trk2col(trk: usize, key: u8) -> Color {
    match (trk % 2, is_white(key)) {
        (0, true) => Color::RGB(0, 255, 255),
        (0, false) => Color::RGB(0, 180, 180),
        (_, true) => Color::RGB(255, 0, 255),
        (_, false) => Color::RGB(180, 0, 180),
    }
}

pub fn draw_keyboard(
    keyboard: &piano_keyboard::Keyboard2d,
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    pressed: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    canvas.set_draw_color(sdl2::pixels::Color::RGB(100, 100, 100));
    canvas.clear();
    //let rec = canvas.viewport();
    let (col_white, col_black) = if pressed {
        (Color::RGB(100, 255, 255), Color::RGB(50, 150, 150))
    } else {
        (Color::RGB(200, 200, 200), Color::RGB(0, 0, 0))
    };

    for (col, rects) in vec![
        (col_white, keyboard.white_keys(true)),
        (col_black, keyboard.black_keys()),
    ]
    .drain(..)
    {
        canvas.set_draw_color(col);
        for rect in rects.into_iter() {
            let rec = sdl2::rect::Rect::new(
                rect.x as i32,
                rect.y as i32,
                rect.width as u32,
                rect.height as u32,
            );
            canvas.fill_rect(rec)?;
        }
    }
    Ok(())
}

pub fn get_pressed_key_rectangles(
    keyboard: &piano_keyboard::Keyboard2d,
    height_offset: u32,
    pos_us: i64,
    show_events: &Vec<(u64, usize, midi_sequencer::MidiEvent)>,
) -> Vec<DrawCommand> {
    let nr_of_keys = keyboard.right_white_key - keyboard.left_white_key + 1;
    let mut pressed = vec![0; nr_of_keys as usize];
    let left_key = keyboard.left_white_key;

    for (time, _, evt) in show_events.iter() {
        // TODO: This needs more work. Adjacent midi notes are shown continuously...
        if (*time as i64) > pos_us {
            break;
        }
        if (*time as i64) + 50_000 > pos_us {
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure) => {
                    pressed[(key - left_key) as usize] = *pressure
                }
                _ => (),
            }
        } else {
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure) => {
                    pressed[(key - left_key) as usize] = *pressure
                }
                midi_sequencer::MidiEvent::NoteOff(_channel, key, _) => {
                    pressed[(key - left_key) as usize] = 0
                }
                _ => (),
            }
        }
    }

    let mut highlight = vec![];
    for (el, is_pressed) in keyboard.iter().zip(pressed.iter()) {
        if *is_pressed > 0 {
            let rects = match *el {
                piano_keyboard::Element::WhiteKey {
                    wide: ref r1,
                    small: ref r2,
                    blind: Some(ref r3),
                } => vec![r1, r2, r3],
                piano_keyboard::Element::WhiteKey {
                    wide: ref r1,
                    small: ref r2,
                    blind: None,
                } => vec![r1, r2],
                piano_keyboard::Element::BlackKey(ref r1) => vec![r1],
            };
            for r in rects.into_iter() {
                let src_rect =
                    sdl2::rect::Rect::new(r.x as i32, r.y as i32, r.width as u32, r.height as u32);
                let dst_rect = sdl2::rect::Rect::new(
                    r.x as i32,
                    (r.y as u32 + height_offset) as i32,
                    r.width as u32,
                    r.height as u32,
                );
                let cmd = DrawCommand::CopyToScreen {
                    src_texture: 1,
                    src_rect,
                    dst_rect,
                };
                highlight.push(cmd);
            }
        }
    }
    highlight
}

pub fn draw_waterfall(
    keyboard: &piano_keyboard::Keyboard2d,
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    i: u32,
    bottom_row: u32,
    net_rows: u32,
    overlap: u32,
    rows_per_s: u32,
    show_events: &Vec<(u64, usize, midi_sequencer::MidiEvent)>,
) {
    // The waterfall is flowing from top to bottom with SDL having origin top left.
    // Thus every texture has to fill from bottom to top.

    if false {
        let i = (i & 1) as u8 * 40;
        canvas.set_draw_color(sdl2::pixels::Color::RGB(100 + i, 100 + i, 100 + i));
    } else {
        canvas.set_draw_color(sdl2::pixels::Color::RGB(100, 100, 100));
    }
    canvas.clear();

    let left_key = keyboard.left_white_key;
    let mut rect_templates: Vec<sdl2::rect::Rect> = vec![];
    for el in keyboard.iter() {
        let (x, width) = match *el {
            piano_keyboard::Element::WhiteKey {
                wide: _,
                small: ref r1,
                blind: Some(ref r2),
            } => (r1.x.min(r2.x), r1.width + r2.width),
            piano_keyboard::Element::WhiteKey {
                wide: _,
                small: ref r,
                blind: None,
            }
            | piano_keyboard::Element::BlackKey(ref r) => (r.x, r.width),
        };
        rect_templates.push(sdl2::rect::Rect::new(x as i32, 0, width as u32, 0));
    }

    for (i, _el) in keyboard.iter().enumerate() {
        let sel_key = left_key + i as u8;
        let mut opt_start = None;
        let mut opt_end = None;
        for (time, trk, evt) in show_events.iter() {
            let col = trk2col(*trk, sel_key);
            match evt {
                midi_sequencer::MidiEvent::NoteOn(_channel, key, pressure)
                    if *key == sel_key && *pressure > 0 =>
                {
                    opt_start = Some((time * rows_per_s as u64 / 1_000_000) as u32);
                    trace!("{}: {:?}  {:?}", time, evt, opt_start);
                }
                midi_sequencer::MidiEvent::NoteOn(_channel, key, 0)
                | midi_sequencer::MidiEvent::NoteOff(_channel, key, _)
                    if *key == sel_key =>
                {
                    opt_end = Some((time * rows_per_s as u64 / 1_000_000) as u32);
                    trace!("{}: {:?}  {:?}", time, evt, opt_end);
                }
                _ => continue,
            }
            match (opt_start, opt_end) {
                (Some(start), Some(end)) => {
                    let top_row = bottom_row + net_rows + overlap - 1;
                    if start > top_row {
                        continue;
                    }
                    if end <= bottom_row {
                        continue;
                    }
                    trace!("start/end = {}/{}", start, end);
                    let start_row = start.max(bottom_row);
                    let end_row = end.min(top_row);
                    trace!("{} {}", start_row, end_row);
                    let height = end_row - start_row + 1;
                    let tex_y = top_row - end_row; // flip

                    let mut rec = rect_templates[i].clone();
                    rec.set_y(tex_y as i32);
                    rec.set_height(height);
                    trace!("Need draw: {:?}", rec);
                    let rounding = rec.width() as i16 / 2 - 1;
                    // later change to draw two circles and a rectangle
                    canvas
                        .rounded_box(
                            rec.left() as i16,
                            rec.bottom() as i16 - rounding as i16 / 2 + 1,
                            rec.right() as i16,
                            rec.top() as i16 + rounding as i16 / 2 - 1,
                            rounding,
                            col,
                        )
                        .unwrap();
                    opt_start = None;
                    opt_end = None;
                }
                (None, Some(_)) => {
                    opt_end = None;
                    warn!("Note Off with Note On should not happen")
                }
                _ => (),
            }
        }
    }
}

pub fn copy_waterfall_to_screen(
    n: usize,
    wf_width: u32,
    wf_height: u32,
    net_rows: u32,
    overlap: u32,
    rows_per_s: u32,
    pos_us: i64,
) -> Vec<DrawCommand> {
    trace!(
        "copy_wf_to_screen: n={} wf_width={} wf_height={}",
        n,
        wf_width,
        wf_height
    );
    trace!(
        "       net_rows={} overlap={} rows_per_s={} pos_us={}",
        net_rows,
        overlap,
        rows_per_s,
        pos_us
    );
    // if pos_us = 0, then first texture bottom need to reach keyboard
    // Thus
    //      src_rect.x=net_rows+overlap-height,src_rect.height=height
    //      dst_rect.x=0,dst_rect.height=height

    // rows to display
    //    top/bottom as on display visible, with y(top) < y(bottom)
    let wf_row_top = pos_us * rows_per_s as i64 / 1_000_000;
    let wf_row_bottom = wf_row_top + wf_height as i64 - 1;

    let mut commands = vec![];
    for i in 0..n {
        // Texture i covers these total rows
        let tex_row_top = (i as u32 * net_rows) as i64;
        let tex_row_bottom = tex_row_top + net_rows as i64 - 1;

        // The intersection with the canvas top/bottom row is the region to copy
        let copy_row_top = wf_row_top.max(tex_row_top);
        let copy_row_bottom = wf_row_bottom.min(tex_row_bottom);
        debug!(
            "Texture {}: Overlap texture {}-{}<>requested area {}-{} => {}-{}",
            i,
            tex_row_top,
            tex_row_bottom,
            wf_row_top,
            wf_row_bottom,
            copy_row_top,
            copy_row_bottom
        );

        // If the intersection does not contain rows, continue with next texture
        if copy_row_top > copy_row_bottom {
            continue;
        }

        #[cfg(test)]
        println!(
            "Texture {}: Overlap texture {}-{}<>requested area {}-{} => {}-{}",
            i,
            tex_row_top,
            tex_row_bottom,
            wf_row_top,
            wf_row_bottom,
            copy_row_top,
            copy_row_bottom
        );

        // The number of intersecting rows:
        let cp_height = (copy_row_bottom - copy_row_top + 1) as u32;

        // The distance from wf_row_bottom to copy_row_bottom
        // yields the y_shift
        let y_dst_bottom = wf_height as i64 - (wf_row_bottom - copy_row_bottom) - 1;
        let y_dst_top = (y_dst_bottom - (cp_height as i64 - 1)) as i32;
        let y_dst = wf_height as i32 - y_dst_top - cp_height as i32;
        #[cfg(test)]
        println!(
            "y_dst_bottom={} y_dst_top={}  y_dst={}  bottom={}",
            y_dst_bottom,
            y_dst_top,
            y_dst,
            wf_height - 1
        );

        let y_src = (overlap + net_rows) as i32
            - cp_height as i32
            - (copy_row_top - tex_row_top).max(0) as i32;

        let src_rect = sdl2::rect::Rect::new(0, y_src, wf_width, cp_height);
        let dst_rect = sdl2::rect::Rect::new(0, y_dst, wf_width, cp_height);
        trace!(target: "copy_texture", "Copy {:?}->{:?}", src_rect, dst_rect);
        let cmd = DrawCommand::CopyToScreen {
            src_texture: i + 2,
            src_rect,
            dst_rect,
        };
        commands.push(cmd);
    }
    commands
}
#[cfg(test)]
mod tests {
    use crate::draw_engine;

    #[test]
    fn test_01() {
        let n = 28;
        let wf_width = 4096;
        let wf_height = 1515;
        let net_rows = 907;
        let overlap = 93;
        let rows_per_s = 100;
        let pos_us = 6199732;
        let mut cmds = draw_engine::copy_waterfall_to_screen(
            n, wf_width, wf_height, net_rows, overlap, rows_per_s, pos_us,
        );
        assert_eq!(cmds.len(), 3);
        let mut dst_total_height = 0;
        let last = cmds.pop().unwrap();
        match last {
            draw_engine::DrawCommand::CopyToScreen {
                src_texture,
                src_rect,
                dst_rect,
            } => {
                // sdl bottom is UNDER the rectangle....
                assert_eq!(src_rect.height() as i32 + src_rect.top(), src_rect.bottom());

                // Last texture is on top. So destination y must be 0 and source y max
                assert_eq!(src_texture, 4);
                assert_eq!(src_rect.left(), 0);
                assert_eq!(src_rect.width(), wf_width);
                assert_eq!(dst_rect.left(), 0);
                assert_eq!(dst_rect.width(), wf_width);
                assert_eq!(src_rect.height(), dst_rect.height());
                assert_eq!((overlap + net_rows) as i32, src_rect.bottom());
                dst_total_height += dst_rect.height();
                assert_eq!(dst_rect.top(), 0);
            }
        }
        let second = cmds.pop().unwrap();
        match second {
            draw_engine::DrawCommand::CopyToScreen {
                src_texture,
                src_rect,
                dst_rect,
            } => {
                // Middle texture is in the middle
                assert_eq!(src_texture, 3);
                assert_eq!(src_rect.left(), 0);
                assert_eq!(src_rect.width(), wf_width);
                assert_eq!(dst_rect.left(), 0);
                assert_eq!(dst_rect.width(), wf_width);
                assert_eq!(src_rect.height(), dst_rect.height());
                dst_total_height += dst_rect.height();
            }
        }
        let first = cmds.pop().unwrap();
        match first {
            draw_engine::DrawCommand::CopyToScreen {
                src_texture,
                src_rect,
                dst_rect,
            } => {
                // First texture is at the bottom. So destination y must be max
                // and source y equal overlap
                assert_eq!(src_texture, 2);
                assert_eq!(src_rect.left(), 0);
                assert_eq!(src_rect.width(), wf_width);
                assert_eq!(dst_rect.left(), 0);
                assert_eq!(dst_rect.width(), wf_width);
                assert_eq!(src_rect.top(), overlap as i32);
                assert_eq!(src_rect.height(), dst_rect.height());
                dst_total_height += dst_rect.height();
                assert_eq!(dst_rect.bottom(), wf_height as i32);
            }
        }
        assert_eq!(dst_total_height, wf_height);
    }
}
