use std::time::{Duration,Instant};

use font_kit;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::pixels::Color;

use piano_keyboard;

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

pub struct DrawEngine {
    canvas: Option<sdl2::render::Canvas<sdl2::video::Window>>,
}
impl DrawEngine {
    pub fn init(sdl_context: &mut sdl2::Sdl) -> Result<DrawEngine, Box<std::error::Error>> {
        let midi_fname = format!("fname");
        let video_subsystem = sdl_context.video().unwrap();
        let window = video_subsystem
            .window(&format!("Rusthesia: {}", midi_fname), 800, 600)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

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

        let mut canvas = window.into_canvas().build().unwrap();

        Ok(DrawEngine {
            canvas: Some(canvas)
        })
        //Err(Error::new(ErrorKind::Other, "oh no!"))
    }
    pub fn run_engine(&mut self, sdl_context: u32) {
    }
    pub fn draw(&mut self, sdl_context: u32) -> Result<(), Box<std::error::Error>> {
        let mut paused = false;
        let mut pos_us = 0;
        let mut scale_1000 = 1000;
        let mut finger_msg = format!("----");
        let maxtime = 100;
        let opt_font: Option<sdl2::ttf::Font> = None;
        let (left_key,right_key) = (21,108);

        let mut canvas = self.canvas.take().unwrap();
        let texture_creator = canvas.texture_creator();

        canvas.set_draw_color(Color::RGB(0, 255, 255));
        canvas.clear();
        canvas.present();

        let mut opt_waterfall: Option<sdl2::render::Texture> = None;
        let mut opt_last_draw_instant: Option<Instant> = None;
        loop {
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
                    //lines.push(format!("shift = {}", shift_key));
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
        }
    }
}
