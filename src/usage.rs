use clap::{crate_authors, crate_version};
use clap::{App, Arg};
use indoc::indoc;

pub fn usage() -> clap::ArgMatches<'static> {
    App::new("Rusthesia")
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .about(indoc!(
            "
                                    Reads midi files and creates piano notes waterfall.

                                    Valid key commands, while playing:
                                        <Cursor-Left>   Transpose half tone lower
                                        <Cursor-Right>  Transpose half tone higher
                                        <Cursor-Up>     Go back some time
                                        <Space>         Pause/continue playing
                                        "
        ))
        .arg(
            Arg::with_name("transpose")
                .short("t")
                .long("transpose")
                .takes_value(true)
                .default_value("0")
                .help("Set number of note steps to transpose"),
        )
        .arg(
            Arg::with_name("play")
                .required_unless("list")
                .short("p")
                .long("play-tracks")
                .takes_value(true)
                .multiple(true)
                .help("Output these tracks as midi"),
        )
        .arg(
            Arg::with_name("show")
                .required_unless("list")
                .short("s")
                .long("show-tracks")
                .takes_value(true)
                .multiple(true)
                .help("Show the tracks as falling notes"),
        )
        .arg(
            Arg::with_name("list")
                .short("l")
                .long("list-tracks")
                .help("List the tracks in the midi file"),
        )
        .arg(
            Arg::with_name("RD64")
                .long("rd64")
                .help("Select 64 key Piano like Roland RD-64"),
        )
        .arg(
            Arg::with_name("MIDI")
                .help("Sets the midi file to use")
                .required(true)
                .index(1),
        )
        .arg(Arg::with_name("verbose").multiple(true).short("v"))
        .arg(Arg::with_name("debug").short("d"))
        .get_matches()
}
