# Rusthesia

Rusthesia is till now just a hack to play a midi file, created from Logic Pro/X, and display a window with falling notes down onto a piano. Thus hardcoded for example is to use track 1 and 2 as piano input.

The midi file can be transposed in half notes in realtime by using shift left/right key. Playing can be paused by space.

Synthesizer is not included. Instead midi messages will be sent by core audio on a mac. No idea for other operation systems. Screen output uses sdl2.

Here screenshot for current version:
![Screensho](screenshot.png)

## Installation

```
cargo install rusthesia
```

## Preparation



## Usage

For help just execute

```
rusthesia -h
```

## Todo

- Refactoring and code quality
- Create video
- Speed up by using e.g. one large sdl2 surface
- Nicer looking output
- Different color for the channels (left/right)
- Native macos app with fruitbasket
- and more...

## Final Words

The application works, but still this is a quick hack. In future refactoring will be necessary.