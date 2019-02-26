#!/bin/bash
cargo watch -w src -w Cargo.toml -w sdl2_timing -x build -x test -x doc
