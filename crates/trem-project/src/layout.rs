//! On-disk package layout conventions for a `trem.toml` project.

/// Root manifest file for a trem project package.
pub const MANIFEST_FILE: &str = "trem.toml";
/// Scene documents live here.
pub const SCENES_DIR: &str = "scenes";
/// Canonical authored clip files live here.
pub const CLIPS_DIR: &str = "clips";
/// Graph definitions live here.
pub const GRAPHS_DIR: &str = "graphs";
/// Sample/audio assets live here.
pub const SAMPLES_DIR: &str = "samples";
/// Imported/exported MIDI assets live here.
pub const MIDIS_DIR: &str = "midis";
