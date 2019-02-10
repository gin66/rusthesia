use std::cmp::Ordering;
use std::fs::File;
use std::io::Read;
use std::io::{Error, ErrorKind};
use std::iter::Iterator;

pub struct TrackState<'m> {
    trk_number: usize,
    trk_iter: std::slice::Iter<'m, midly::Event<'m>>,
    time: u32,
    evt: Option<&'m midly::Event<'m>>
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
        if self.evt.is_some() != other.evt.is_some() {
            if self.evt.is_some() {
                Ordering::Less
            }
            else {
                Ordering::Greater
            }
        }
        else {
            self.sort_key().cmp(&other.sort_key())
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
    type Item = (u32, usize, &'m midly::EventKind<'m>);
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
                return Some( (time,trk_number,&evt.kind) );
            }
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
}

#[cfg(test)]
mod tests {
    use crate::midi_container;

    #[test]
    fn test_01() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(container.iter().count(),2423);
        //for evt in container.iter() {
        //    println!("{:?}", evt);
        //}
    }

    #[test]
    fn test_02() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(container.iter().filter(
                |(_time,track_id,_evt)| *track_id == 0).count(),6);
    }

    #[test]
    fn test_03() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(container.iter().filter(
                |(_time,track_id,_evt)| *track_id == 1).count(),1679);
    }

    #[test]
    fn test_04() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        assert_eq!(container.iter().filter(
                |(_time,track_id,_evt)| *track_id == 2).count(),2423-6-1679);
    }

    #[test]
    fn test_05() {
        let midi_fname = "Marche_aux_Flambeaux.mid";
        let smf_buf = midly::SmfBuffer::open(&midi_fname).unwrap();
        let container = midi_container::MidiContainer::from_buf(&smf_buf).unwrap();
        match container.header().timing {
            midly::Timing::Metrical(t) => assert_eq!(t.as_int(),384),
            _ => panic!("wrong type")
        }
    }
}

