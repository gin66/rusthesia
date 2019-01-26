use std::thread::sleep;
use std::time::Duration;
use std::io::{stdin, stdout, Write};
use std::error::Error;

use simple_logging;
use log::LevelFilter;

use midly;
use midir::MidiOutput;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err.description())
    }
}

fn run() -> Result<(), Box<Error>> {
    simple_logging::log_to_stderr(LevelFilter::Trace);

    let midi = include_bytes!("../Forrest Gump_Feather Theme.mid");
    let smf: midly::Smf<Vec<midly::Event>>=midly::Smf::read(midi).unwrap();
    println!("{:#?}", smf);

    let midi_out = MidiOutput::new("My Test Output")?;
    
    // Get an output port (read from console if multiple are available)
    let out_port = match midi_out.port_count() {
        0 => return Err("no output port found".into()),
        1 => {
            println!("Choosing the only available output port: {}", midi_out.port_name(0).unwrap());
            0
        },
        _ => {
            println!("\nAvailable output ports:");
            for i in 0..midi_out.port_count() {
                println!("{}: {}", i, midi_out.port_name(i).unwrap());
            }
            print!("Please select output port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            input.trim().parse()?
        }
    };
    
    println!("\nOpening connection");
    let mut conn_out = midi_out.connect(out_port, "midir-test")?;
    println!("Connection open");
    println!("{:#?}",smf.header.timing);
    let tref = match smf.header.timing {
        midly::Timing::Metrical(x) => x.as_int() as u32,
        midly::Timing::Timecode(x,y) => 1
    };
    let mut tracks = vec![];
    tracks.push( (0,smf.tracks[1].iter(), None) );
    tracks.push( (0,smf.tracks[2].iter(), None) );

    let mut realtime = 0;
    loop {
        if tracks.len() > 1 {
            tracks.sort_by_key(|x| u32::max_value()-x.0);
        }
        if let Some( (t,mut t_iter, m) ) = tracks.pop() {
            if t > realtime {
                sleep(Duration::from_millis((t-realtime) as u64));
                realtime = t;
            }
            match m {
                Some(midly::MidiMessage::NoteOn(p1,p2)) =>
                    conn_out.send(&[0x90, p1.as_int(), p2.as_int()]).unwrap(),
                Some(midly::MidiMessage::NoteOff(p1,p2)) =>
                    conn_out.send(&[0x80, p1.as_int(), p2.as_int()]).unwrap(),
                m => 
                    println!("=> {:#?}",m)
            }
            if let Some(ev) = t_iter.next() {
                let dt = ev.delta.as_int() * tref / 120 / 4;
                println!("dt={} ms",dt);
                if let midly::EventKind::Midi{channel: _c,message: m} = ev.kind {
                    tracks.push( (t+dt,t_iter, Some(m)) );
                }
                else {
                    println!("=> {:#?}",ev);
                    tracks.push( (t+dt,t_iter, None) );
                }
            }
        }
    }
    sleep(Duration::from_millis(150));
    println!("\nClosing connection");
    // This is optional, the connection would automatically be closed as soon as it goes out of scope
    conn_out.close();
    println!("Connection closed");
    Ok(())
}
