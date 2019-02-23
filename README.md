# Rusthesia

Rusthesia is till now just a hack to play a midi file, created from Logic Pro/X, and display a window with falling notes down onto a piano.


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

To get info about the event loop in regard to timing debug flags can be added:
```
> rusthesia Marche_aux_Flambeaux.mid -p 1 -vvv -d eventloop
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

#### fluidsynth

Good choice too is to use fluidsynth

```bash
brew install fluid-synth
```

For playing need still a soundfont .sf2 file, which can be found in the internet. Start synthesizer with:

```bash
fluidsynth your_soundfound.sf2
```

Now can start rusthesia and the midi-output should appear.

```
Available output ports:
0: IAC-Treiber IAC-Bus 1
1: FluidSynth virtual port (3776)
```

Just enter 1 for this case.

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

Performance measurement yields:

```
cargo run --release Marche_aux_Flambeaux.mid -p 0 1 2 -s 0 1 2 -d eventloop -vvvv
Sleep time 2 ms - 1 times
Sleep time 7 ms - 1 times
Sleep time 14 ms - 2 times
Sleep time 15 ms - 1 times
Sleep time 23 ms - 1 times
Sleep time 24 ms - 6 times
Sleep time 25 ms - 33 times
Sleep time 26 ms - 153 times
Sleep time 27 ms - 118 times
Sleep time 31 ms - 1 times
Sleep time 37 ms - 1 times
min=     0us avg=     2us max=    74us control at loop start
min=     0us avg=     0us max=    50us keyboard built
min=     0us avg=     6us max=  1987us keyboard drawn
min=     4us avg=    17us max=    72us canvas cleared
min=    15us avg=    34us max=    91us copy keyboard to canvas
min=     0us avg=    81us max= 22171us waterfall and pressed keys drawn
min=     1us avg=     9us max=   488us event loop
min=  2299us avg= 26602us max= 37510us sleep
min= 12130us avg= 13410us max= 37508us canvas presented
```

Same for macos:
```
Sleep time 18 ms - 1 times
Sleep time 20 ms - 1 times
Sleep time 22 ms - 1 times
Sleep time 29 ms - 1 times
Sleep time 32 ms - 1 times
Sleep time 34 ms - 1 times
Sleep time 35 ms - 5 times
Sleep time 36 ms - 22 times
Sleep time 37 ms - 275 times
Sleep time 38 ms - 39 times
min=     2us avg=     3us max=    86us control at loop start
min=     1us avg=     2us max=   105us keyboard built
min=     0us avg=     5us max=  1811us keyboard drawn
min=     7us avg=    11us max=    50us canvas cleared
min=     9us avg=    18us max=  1094us copy keyboard to canvas
min=     0us avg=   188us max= 59459us waterfall and pressed keys drawn
min=    45us avg=   662us max=162006us event loop
min= 19194us avg= 38067us max= 39718us sleep
min=  1126us avg=  1623us max= 12602us canvas presented
```

On macos canvas present is on average 8 times faster. In addition on linux the waterfall has several flaws. 
Not sure if this is due to intel graphic driver or sdl library or ...

Funnily on macos the event loop can be blocked by poll_event for a long time, which is weird. Luckily this appears to happen only for Window events, which are seldom.

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
