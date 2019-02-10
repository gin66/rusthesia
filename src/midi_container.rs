use std::fs::File;
use std::io::Read;
use std::io::{Error, ErrorKind};

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
}

pub struct MidiContainer<'m> {
    raw_midi: Vec<u8>,
    opt_smf: Option<midly::Smf<'m,Vec<midly::Event<'m>>>>,
    track_parser: Vec<TrackState<'m>>,
}

impl<'m> MidiContainer<'m> {
    pub fn from_file(midi_fname: &str) -> Result<MidiContainer<'m>,Error> {
        let mut raw_midi = vec!{};
        let mut f = File::open(midi_fname)?;
        f.read_to_end(&mut raw_midi)?;
        Ok(MidiContainer {
            raw_midi,
            opt_smf: None,
            track_parser: vec![],
        })
    }
    pub fn read_file(&'m mut self) -> Result<(),Error> {
        self.opt_smf = Some(midly::Smf::read(&self.raw_midi)
                                .map_err(|e|Error::new(ErrorKind::Other,format!("{:?}",e)))?);
        Ok(())
    }
    pub fn init_play(&'m mut self) {
        for i in 0..self.opt_smf.as_ref().unwrap().tracks.len() {
            let ts = TrackState::new(i, self.opt_smf.as_ref().unwrap().tracks[i].iter());
            self.track_parser.push(ts);
        }
    }
}
