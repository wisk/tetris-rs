# tetris-rs

A Tetris game written in Rust with an NES-inspired visual design and a pixel-art St. Basil's Cathedral background.

![Rust](https://img.shields.io/badge/language-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- Classic Tetris gameplay with 7-bag randomizer for fair piece distribution
- AI auto-play mode that favors Tetris (4-line) clears
- NES-style UI with gold ornamental borders and pixel-art font
- Detailed St. Basil's Cathedral pixel art with colorful striped onion domes
- Animated snowfall, twinkling stars, and glowing cathedral windows
- Special flash effect on Tetris (4-line) clears
- Ghost piece preview

## Controls

| Key | Action |
|-----|--------|
| ← → / A D | Move piece left/right |
| ↑ / W | Rotate piece |
| ↓ / S | Soft drop |
| Space | Hard drop |
| T | Toggle AI auto-play |
| R | Restart game |
| Escape | Quit |

## Building & Running

```bash
cargo run
```

For an optimized build:

```bash
cargo run --release
```

## Dependencies

- [minifb](https://crates.io/crates/minifb) - Minimal framebuffer window
- [rand](https://crates.io/crates/rand) - Random number generation

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
