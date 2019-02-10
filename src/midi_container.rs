use std::fs::File;
use std::io::Read;
use std::io::{Error, ErrorKind};
use std::iter::Iterator;
use std::cmp::Ordering;

pub struct TrackState<'m> {
    trk_number: usize,
    trk_iter: std::slice::Iter<'m,midly::Event<'m>>,
}
impl<'m> TrackState<'m> {
    fn new(trk_number: usize, trk_iter: std::slice::Iter<'m,midly::Event<'m>>) -> TrackState<'m> {
        TrackState {
            trk_number,
            trk_iter,
        }
    }
    fn sort_key(&self) -> usize {
        self.trk_number
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
        self.sort_key().cmp(&other.sort_key())
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
    pub fn add_track(&mut self, trk_number: usize, trk_iter: std::slice::Iter<'m,midly::Event<'m>>) {
        let ts = TrackState::new(trk_number, trk_iter);
        self.track_parsers.push(ts);
    }        
}
impl<'m> Iterator for MidiIterator<'m> {
    type Item = (usize, usize, midly::Event<'m>);
    fn next(&mut self) -> Option<Self::Item> {
        self.track_parsers.sort();
        None
    }
}

pub struct MidiContainer<'m> {
    raw_midi: Vec<u8>,
    opt_smf: Option<midly::Smf<'m,Vec<midly::Event<'m>>>>,
}
impl<'m> MidiContainer<'m> {
    pub fn from_file(midi_fname: &str) -> Result<MidiContainer<'m>,Error> {
        let mut raw_midi = vec!{};
        let mut f = File::open(midi_fname)?;
        f.read_to_end(&mut raw_midi)?;
        Ok(MidiContainer {
            raw_midi,
            opt_smf: None,
        })
    }
    pub fn read_file(&'m mut self) -> Result<(),Error> {
        self.opt_smf = Some(midly::Smf::read(&self.raw_midi)
                                .map_err(|e|Error::new(ErrorKind::Other,format!("{:?}",e)))?);
        Ok(())
    }
    pub fn iter(&'m mut self) -> MidiIterator<'m> {
        let mut mi = MidiIterator::new();
        if let Some(ref smf) = self.opt_smf.as_ref() {
            for (i,trk) in smf.tracks.iter().enumerate() {
                mi.add_track(i, trk.iter());
            }
        }
        mi
    }
}

