# Rusthesia

Rusthesia is till now just a hack to play a midi file, created from Logic Pro/X, and display a window with falling notes down onto a piano.


========================

# The development version is under rework. Waterfall and key shift is not yet reimplemented.

========================

The midi file can be transposed in half notes in realtime by using shift left/right key. Playing can be paused by space.


### Audio

Synthesizer is not included. Instead midi messages will be sent via core audio. Logic Pro/X can be used for playing the midi, but has to be set up accordingly.

No idea for other operation systems, if it works or how to do. 

### Video

Screen output uses sdl2.

### Screenshot

![Screenshot](screenshot.png)

## Preparation

The sdl2 libraries need to be installed. On macos this can be done by:

```
brew install sdl2 sdl2_gfx
```

## Installation

```
cargo install rusthesia
```

## Usage

For help just execute

```
rusthesia -h
```

As an example the midi-file 
[Marche_aux_Flambeaux.mid](http://www.mutopiaproject.org/cgibin/make-table.cgi?Instrument=Harmonium)
is included. As per that website, this file is in the public domain.

First list the available tracks:
```
> rusthesia Marche_aux_Flambeaux.mid -l
Track 0:
  Text: Creator: GNU LilyPond 2.8.7
  Text: Generated automatically by: GNU LilyPond 2.8.7
  Text: at Mon Oct 16 20:41:39 2006
  Text: at Mon Oct 16 20:41:39 2006
  Track name: Track 0
Track 1:
  Track name: upper
  Instrument name: accordion
Track 2:
  Track name: lower
  Instrument name: accordion
```

For playing and displaying all tracks use:
```
> rusthesia Marche_aux_Flambeaux.mid -p 0 1 2 -s 0 1 2
```

In order to play the lower and show only the upper, use the following command:
```
> rusthesia Marche_aux_Flambeaux.mid -p 1 -s 2
```

Only use the midi player function without graphic output:
```
> rusthesia Marche_aux_Flambeaux.mid -p 1
```

## Todo

Todo list is managed under [projects](https://github.com/gin66/rusthesia/projects)

## Synthesizer

### macos

#### Logic Pro/X

Have not been able to use Logic Pro/X as a synthesizer with channels assigned to different instruments. Still keep looking for the needed hidden feature.

#### MainStage

Works, but need to create a concert with keyboard per channel, which is not very convenient.

#### Connected piano (Roland RD-64) with synthesizer

As rusthesia offered to use my Roland RD-64 as output, I have given it a try.
Very positively surprised, that it works pretty well. Even can play along by hand to the song.

### Linux

As per info from Samuel Da Mota, the code works on linux:

> No need to change a single
> line of code or configuration whatsoever. One only need to install the
> libsdl2-dev and libsd2-gfx-dev packages for your project to build. And
> then to install a system wide midi sequencer such as timidity and run
> it (using timidity -iA) to get music being played.

In the meantime have tried on deepin linux, which is IMHO based on Debian.
This steps to be executed for compilation:

```
sudo apt install librtaudio-dev libsdl2-2.0 cmake libfreetype6-dev libsdl2-dev libsdl2-gfx-dev libsdl2-ttf-dev libfontconfig1-dev
```

Unfortunatly it does not work for these issues:

* Long pause before windows comes up
* Super slow
* No output if using timidity -A

Need further debugging.

## Data to Marche_aux_Flambeaux.mid

* 115 bars
* 4/4 bar
* 110 beats per minute (as per Logic Pro/X)
* duration 4 minutes 8 seconds
* First note starts at bar 2.0
* Last note ends at bar 114.3

## Solutions for known issues

### Linux

If the application outputs lots of fontconfig errors, then there could be a libfontconfig mismatch. Please check, if pkg-config is able to find fontconfig:

```
pkg-config --list-all|grep fontconfig
```

If there is no output, then crate *servo-fontconfig-sys* will build its own version, which can be incompatible. Installation of libconfig1-dev has fixed this.

## License

The attached LICENSE file defines the license for the code of this crate only - specifically before compiling or linking. The resulting binary after linking may be problematic in regard to license incompatibilities of included crates.

From current point of view to be checked:
    BSD-3-Clause (1): simple-logging
    ISC (1): rdrand
    N/A (2): fuchsia-cprng
    Unlicense (1): midly

AFAIK is incompatible:
    GPL-3.0-or-later (1): sdl2-unifont

Consequently automated builds resulting in a public available binary cannot be set up for now.

## Final Words

The application works, but still this is a quick hack and not polished for code review ;-)

