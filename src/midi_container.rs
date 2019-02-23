use std::cmp::Ordering;
use std::io::{Error, ErrorKind};
use std::iter::Iterator;

use log::*;

pub struct TrackState<'m> {
    trk_number: usize,
    trk_iter: std::slice::Iter<'m, midly::Event<'m>>,
    time: u32,
    evt: Option<&'m midly::Event<'m>>,
}
impl<'m> TrackState<'m> {
    fn new(trk_number: usize, trk_iter: std::slice::Iter<'m, midly::Event<'m>>) -> TrackState<'m> {
        TrackState {
            trk_number,
            trk_iter,
            time: 0,
            evt: None,
        }
    }
    fn sort_key(&self) -> u32 {
        self.time
    }
}
impl<'m> std::cmp::PartialEq for TrackState<'m> {
    fn eq(&self, other: &Self) -> bool {
        self.sort_key() == other.sort_key()
    }
}
impl<'m> std::cmp::Eq for TrackState<'m> {}
impl<'m> std::cmp::PartialOrd for TrackState<'m> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<'m> std::cmp::Ord for TrackState<'m> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.evt.is_some(), other.evt.is_some()) {
            (false, false) => Ordering::Equal,
            (false, true) => Ordering::Less,
            (true, false) => Ordering::Greater,
            (true, true) => match self.sort_key().cmp(&other.sort_key()) {
                Ordering::Equal => match self.evt.as_ref().unwrap().kind {
                    midly::EventKind::Meta(_) => Ordering::Less,
                    _ => match other.evt.as_ref().unwrap().kind {
                        midly::EventKind::Meta(_) => Ordering::Greater,
                        _ => Ordering::Equal,
                    },
                },
                o => o,
            },
        }
    }
}

pub struct MidiIterator<'m> {
    track_parsers: Vec<TrackState<'m>>,
}
impl<'m> MidiIterator<'m> {
    pub fn new() -> MidiIterator<'m> {
        MidiIterator {
            track_parsers: vec![],
        }
    }
    pub fn add_track(
        &mut self,
        trk_number: usize,
        trk_iter: std::slice::Iter<'m, midly::Event<'m>>,
    ) {
        let ts = TrackState::new(trk_number, trk_iter);
        self.track_parsers.push(ts);
    }
}
impl<'m> Iterator for MidiIterator<'m> {
    type Item = (u64, usize, &'m midly::EventKind<'m>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.track_parsers.len() == 0 {
                return None;
            }
            self.track_parsers.sort();
            let mut p = self.track_parsers.remove(0);
            let trk_number = p.trk_number;
            let time = p.time;
            let opt_evt = p.evt.take();
            if let Some(m) = p.trk_iter.next() {
                p.time += m.delta.as_int();
                p.evt = Some(m);
                self.track_parsers.push(p);
            }
            if let Some(evt) = opt_evt {
                return Some((time as u64, trk_number, &evt.kind));
            }
        }
    }
}

pub struct MidiTimedIterator<'m> {
    opt_midi_iter: Option<MidiIterator<'m>>,
    timing: &'m midly::Timing,
    timebase: Option<u64>,
    current_time_us: u64,
    last_tick: u32,
}
impl<'m> MidiTimedIterator<'m> {
    fn update_timebase(&mut self, tempo: u32) {
        let ppqn = match self.timing {
            // http://www.onicos.com/staff/iz/formats/smf006.html
            // http://midiwonder.com/midifile.html
            //
            // tempo = 24ths of a microsecond per MIDI clock
            midly::Timing::Metrical(x) => x.as_int() as u32,
            midly::Timing::Timecode(_x, _y) => panic!("Timecode not implemented"),
        };
        let bpm = 60_000_000 / tempo as u64;

        self.timebase = Some(60_000_000 / ppqn as u64 / bpm);
    }
}
impl<'m> Iterator for MidiTimedIterator<'m> {
    type Item = (u64, usize, &'m midly::EventKind<'m>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.opt_midi_iter.is_none() {
                return None;
            }
            let opt_tuple = self.opt_midi_iter.as_mut().unwrap().next();
            if let Some((time, trk, evt_kind)) = opt_tuple {
                let dt = time - self.last_tick as u64;
                if dt > 0 {
                    self.last_tick = time as u32;
                    self.current_time_us += dt as u64 * self.timebase.unwrap();
                }
                match evt_kind {
                    &midly::EventKind::Meta(midly::MetaMessage::Tempo(tmp)) => {
                        self.update_timebase(tmp.as_int());
                    }
                    _ => {
                        return Some((self.current_time_us, trk, evt_kind));
                    }
                }
            } else {
                self.opt_midi_iter = None;
            }
        }
    }
}

impl<'m> MidiIterator<'m> {
    pub fn timed(self, timing: &'m midly::Timing) -> MidiTimedIterator<'m> {
        MidiTimedIterator {
            opt_midi_iter: Some(self),
            timing,
            timebase: None,
            current_time_us: 0,
            last_tick: 0,
        }
    }
}

pub struct MidiContainer<'m> {
    smf: midly::Smf<'m, Vec<midly::Event<'m>>>,
}
impl<'m> MidiContainer<'m> {
    pub fn from_buf(smf_buf: &'m midly::SmfBuffer) -> Result<MidiContainer<'m>, Error> {
        Ok(MidiContainer {
            smf: smf_buf
                .parse()
                .map_err(|e| Error::new(ErrorKind::Other, format!("{:?}", e)))?,
        })
    }
    pub fn iter(&'m self) -> MidiIterator<'m> {
        let mut mi = MidiIterator::new();
        for (i, trk) in self.smf.tracks.iter().enumerate() {
            mi.add_track(i, trk.iter());
        }
        mi
    }
    pub fn header(&'m self) -> &midly::Header {
        &self.smf.header
    }
    pub fn nr_of_tracks(&'m self) -> usize {
        self.smf.tracks.len()
    }
}

pub fn list_command(quiet: bool, midi_fname: &str) -> Result<(), Box<std::error::Error>> {
    let smf_buf = midly::SmfBuffer::open(&midi_fname)?;
    let container = MidiContainer::from_buf(&smf_buf)?;
    if !quiet {
        for _evt in container.iter() {
            //trace!("{:?}", evt);
        }
        for evt in container.iter().timed(&container.header().timing) {
            trace!("timed: {:?}", evt);
        }
    }
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::midi_container;

    #[test]
    fn test_01() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(container.nr_of_tracks(), 3);
        assert_eq!(container.iter().count(), 2423);
        //for evt in container.iter() {
        //    println!("{:?}", evt);
        //}
    }

    #[test]
    fn test_02() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(
            container
                .iter()
                .filter(|(_time, track_id, _evt)| *track_id == 0)
                .count(),
            6
        );
    }

    #[test]
    fn test_03() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(
            container
                .iter()
                .filter(|(_time, track_id, _evt)| *track_id == 1)
                .count(),
            1679
        );
    }

    #[test]
    fn test_04() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(
            container
                .iter()
                .filter(|(_time, track_id, _evt)| *track_id == 2)
                .count(),
            2423 - 6 - 1679
        );
    }

    #[test]
    fn test_05() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        match container.header().timing {
            midly::Timing::Metrical(t) => assert_eq!(t.as_int(), 384),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn test_06() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        let mut last_time = 0;
        for (time, _, _) in container.iter() {
            assert!(last_time <= time);
            last_time = time;
        }

        assert_eq!(last_time, 174720);
    }

    #[test]
    fn test_11() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(
            container.iter().timed(&container.header().timing).count(),
            2421
        ); // 2 Tempo events should be filtered
    }
    #[test]
    fn test_16() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        let mut last_time_us = 0;
        for (time_us, _, _) in container.iter().timed(&container.header().timing) {
            assert!(last_time_us <= time_us);
            last_time_us = time_us;
        }

        assert_eq!(last_time_us, 248_102_400);
    }

}
