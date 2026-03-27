# Demos

- `demos/easybeat/` — authored `trem-project` package with scene, clips, and standard-node graph docs

For planar WAV/FLAC I/O, see **`trem-mio`** examples:

- `cargo run -p trem-mio --example save_planar_wav` — write stereo float WAV
- `cargo run -p trem-mio --example read_planar` — read WAV/FLAC (or a built-in demo file)
- `cargo run -p trem-mio --example roundtrip_memory` — encode/decode WAV in memory
- `cargo run -p trem-mio --example write_flac` — write stereo FLAC
