use log::*;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::app_control::AppControl;

pub fn process_event(event: Event, control: &mut AppControl) -> bool {
    match event {
        Event::Window { win_event, .. } => {
            trace!("Unprocessed window Event: {:?}", win_event);
        }
        Event::Quit { .. }
        | Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } => return false,
        Event::KeyDown {
            keycode: Some(Keycode::Space),
            ..
        } => {
            control.toggle_play();
        }
        Event::KeyDown {
            keycode: Some(Keycode::Plus),
            ..
        } => {
            control.modify_scaling(true);
        }
        Event::TextInput {
            text: ref key,
            ..
        } if key == &"+".to_string() => {
            control.modify_scaling(true);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Minus),
            ..
        } => {
            control.modify_scaling(false);
        }
        Event::TextInput {
            text: ref key,
            ..
        } if key == &"-".to_string() => {
            control.modify_scaling(false);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Up),
            ..
        } => {
            control.change_position(true);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Down),
            ..
        } => {
            control.change_position(false);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Left),
            ..
        } => {
            control.tune_up(false);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Right),
            ..
        } => {
            control.tune_up(true);
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
            trace!("Finger {} {}", y, num_fingers);
            if num_fingers == 2 {
                control.two_finger_scroll_start(y);
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
            control.finger_touch();
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
            control.finger_up();
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
    true
}
