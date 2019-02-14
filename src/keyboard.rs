
struct Keyboard {
    left_white_key: u8,
    right_white_key: u8,
    width: u16,
    height: u16,
}

struct KeyboardBuilder {
    left_white_key: u8,
    right_white_key: u8,
    width: u16,
    max_height: u16,

    white_key_wide_width: u32,
    white_key_small_width_cde: u32,
    white_key_small_width_fb: u32,
    white_key_small_width_ga: u32,

    black_key_width: u32,
    black_key_height: u32,

    white_key_height: u32,
    white_key_small_height: u32,
    white_key_wide_height: u32,
}
impl KeyboardBuilder {
    pub fn new() -> KeyboardBuilder {
        // 88 note piano range from A0 to C8
        KeyboardBuilder {
            left_white_key: 21,
            right_white_key: 108,
            width: 640,
            max_height: 480,

            // http://www.rwgiangiulio.com/construction/manual/layout.jpg
            // below measures are in µm
            white_key_wide_width: 22_150,
            white_key_small_width_cde: 13_970,
            white_key_small_width_fb: 12_830,
            white_key_small_width_ga: 13_080,

            black_key_width: 11_000,
            black_key_height: 45_000,

            white_key_height: 126_270,
            white_key_small_height: 80_000,
            white_key_wide_height: 45_000,
        }
    }
    pub fn is_rd64(mut self) -> KeyboardBuilder {
        // RD-64 is A1 to C7
        self.left_white_key = 21+12;
        self.right_white_key = 108-12;
        self
    }
    pub fn set_most_left_right_white_keys(mut self,
                  left_white_key: u8, right_white_key: u8) -> Option<KeyboardBuilder> {
        if !KeyboardBuilder::is_white(left_white_key) {
            None
        }
        else if !KeyboardBuilder::is_white(right_white_key) {
            None
        }
        else {
            self.left_white_key = left_white_key;
            self.right_white_key = right_white_key;
            Some(self)
        }
    }
    pub fn set_width(mut self, width: u16) -> KeyboardBuilder {
        self.width = width;
        self
    }
    pub fn set_max_height(mut self, max_height: u16) -> KeyboardBuilder {
        self.max_height = max_height;
        self
    }
    pub fn is_white(key: u8) -> bool {
        match key % 12 {
            0 | 2 | 4 | 5 | 7 | 9 | 11 => true,
            1 | 3 | 6 | 8 | 10 => false,
            _ => panic!("wrong value"),
        }
    }
    pub fn key_to_white(key: u8) -> u8 {
        let kx = key % 12;
        match kx {
            0 | 2 | 4 | 5 | 7 | 9 | 11 => (kx + 1) / 2 + (key / 12) * 7,
            1 | 3 | 6 | 8 | 10 => kx / 2 + (key / 12) * 7,
            _ => panic!("impossibe"),
        }
    }

    pub fn build(self) -> Keyboard {
        // RD-64 is A1 to C7
        Keyboard {
            left_white_key: self.left_white_key,
            right_white_key: self.right_white_key,
            width: self.width,
            height: self.max_height,
        }
    }
    pub fn get_key_shapes(&mut self, width: u16, height: u16) {
//        let mut black_keys = vec![];
//        let mut white_keys = vec![];
//        let mut black_keys_on = vec![];
//        let mut white_keys_on = vec![];
//        let mut traces = vec![];
//
//        let left_white_key = key_to_white(left_key);
//        let right_white_key = key_to_white(right_key);
//        let nr_white_keys = right_white_key + 1 - left_white_key;
//
//        let white_key_width = rec.width() / nr_white_keys - 1;
//        let black_key_width = white_key_width * 11_00 / 22_15;
//        let white_key_space = 1;
//        let white_key_height = white_key_width * 126_27 / 22_15;
//        let black_key_height = white_key_height * 80 / (80 + 45);
//        let black_cde_off_center = (13_97 + 11_00 - 22_15) * white_key_width / 22_15;
//        let black_fgah_off_center = (13_08 + 11_00 - 22_15) * white_key_width / 22_15;
//        let part_width = (white_key_width + white_key_space) * nr_white_keys - white_key_space;
//        let offset_x = (rec.left() + rec.right() - part_width as i32) / 2
//            - left_white_key as i32 * (white_key_width + white_key_space) as i32;
//        let box_rounding = (black_key_width / 2 - 1) as i16;
//        for key in left_key..=right_key {
//            match key % 12 {
//                0 | 2 | 4 | 5 | 7 | 9 | 11 => {
//                    let nx = key_to_white(key);
//                    let r = sdl2::rect::Rect::new(
//                        offset_x + (nx * white_key_width + nx * white_key_space) as i32,
//                        rec.bottom() - white_key_height as i32,
//                        white_key_width,
//                        white_key_height,
//                    );
//                    traces.push(r.clone());
//                    match timeline[curr_pos].2[(key as i8 + shift_key) as usize] {
//                        NoteState::Pressed(_) | NoteState::Keep(_) => white_keys_on.push(r),
//                        NoteState::Off => white_keys.push(r),
//                    }
//                }
//                1 | 3 | 6 | 8 | 10 => {
//                    // black keys
//                    let nx = key_to_white(key);
//                    let mut left_x = (white_key_width - (black_key_width - white_key_space) / 2
//                        + nx * white_key_width
//                        + nx * white_key_space) as i32;
//                    match key % 12 {
//                        1 => left_x -= black_cde_off_center as i32,
//                        3 => left_x += black_cde_off_center as i32,
//                        6 => left_x -= black_fgah_off_center as i32,
//                        10 => left_x += black_fgah_off_center as i32,
//                        _ => (),
//                    }
//                    let r = sdl2::rect::Rect::new(
//                        offset_x + left_x,
//                        rec.bottom() - white_key_height as i32,
//                        black_key_width,
//                        black_key_height,
//                    );
//                    traces.push(r.clone());
//                    match timeline[curr_pos].2[(key as i8 + shift_key) as usize] {
//                        NoteState::Pressed(_) | NoteState::Keep(_) => black_keys_on.push(r),
//                        NoteState::Off => black_keys.push(r),
//                    }
//                }
//                _ => (),
//            }
//        }
    }
}
