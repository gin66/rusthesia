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
brew install sdl2 sdl2_gfx sdl2_ttf
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

Performance measurement using:

```
cargo run --release Marche_aux_Flambeaux.mid -p 0 1 2 -s 0 1 2 -d eventloop -vvvv
````

without mouse movements/gestures yields:

```
Sleep time 1 ms - 1 times
Sleep time 14 ms - 1 times
Sleep time 15 ms - 1 times
Sleep time 16 ms - 1 times
Sleep time 17 ms - 1 times
Sleep time 22 ms - 24 times
Sleep time 23 ms - 81 times
Sleep time 24 ms - 151 times
Sleep time 25 ms - 285 times
Sleep time 26 ms - 1736 times
Sleep time 27 ms - 4152 times
Sleep time 28 ms - 2 times
Sleep time 37 ms - 1 times
Sleep time 38 ms - 1 times
Lost frames: 2
min=     0us avg=     1us max=    64us control at loop start
min=     0us avg=     1us max=    41us keyboard built
min=     0us avg=     0us max=  1531us keyboard drawn
min=     3us avg=    17us max=    77us canvas cleared
min=    17us avg=    36us max=   173us copy keyboard to canvas
min=     0us avg=    27us max= 24298us waterfall and pressed keys drawn
min=     1us avg=    10us max=   597us event loop
min=  1821us avg= 27083us max= 38550us sleep
min= 11747us avg= 12834us max= 41421us canvas presented
```

Same for macos:
```
Sleep time 6 ms - 1 times
Sleep time 18 ms - 1 times
Sleep time 26 ms - 1 times
Sleep time 27 ms - 4 times
Sleep time 28 ms - 1 times
Sleep time 29 ms - 4 times
Sleep time 30 ms - 3 times
Sleep time 31 ms - 2 times
Sleep time 32 ms - 10 times
Sleep time 33 ms - 9 times
Sleep time 34 ms - 12 times
Sleep time 35 ms - 75 times
Sleep time 36 ms - 484 times
Sleep time 37 ms - 4921 times
Sleep time 38 ms - 1054 times
Sleep time 39 ms - 1 times
Lost frames: 2
min=     2us avg=     4us max=    98us control at loop start
min=     1us avg=     2us max=   115us keyboard built
min=     0us avg=     0us max=  1355us keyboard drawn
min=     7us avg=    11us max=   261us canvas cleared
min=     9us avg=    15us max=   905us copy keyboard to canvas
min=     0us avg=    43us max= 37208us waterfall and pressed keys drawn
min=    36us avg=   149us max= 90750us event loop
min=  7070us avg= 38221us max= 40092us sleep
min=  1058us avg=  1574us max= 22518us canvas presented
```

On macos canvas present is on average 8 times faster. In addition on linux the waterfall has several flaws. 
Not sure if this is due to intel graphic driver or sdl library or ...

Funnily on macos the event loop can be blocked by poll_event for a long time, which is weird. Luckily this appears to happen only for Window events, which are seldom.

Update:
After changing from Intel acceleration mode driver to Intel default driver the measurements are outperforming macos:

```
Sleep time 34 ms - 1 times
Sleep time 37 ms - 1 times
Sleep time 38 ms - 3 times
Sleep time 39 ms - 6330 times
Lost frames: 1
min=     0us avg=     5us max=    58us control at loop start
min=     0us avg=     2us max=    34us keyboard built
min=     0us avg=     0us max=  1831us keyboard drawn
min=     3us avg=    37us max=   393us canvas cleared
min=    10us avg=    65us max=   251us copy keyboard to canvas
min=     0us avg=    41us max= 44905us waterfall and pressed keys drawn
min=     5us avg=    19us max=   326us event loop
min= 34978us avg= 39624us max= 40005us sleep
min=    47us avg=   207us max=   432us canvas presented
```


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
