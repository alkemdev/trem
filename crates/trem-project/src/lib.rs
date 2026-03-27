//! Portable project/package schema for **trem**.
//!
//! This crate is intentionally **UI-agnostic**. It defines:
//!
//! - the root `trem.toml` manifest,
//! - canonical clip documents under `clips/`,
//! - graph documents under `graphs/`,
//! - scene documents under `scenes/`,
//! - semantic machine-agnostic ports,
//! - typed timeline blocks that can reference clips, graphs, samples, and MIDI assets.
//!
//! The TUI, future GUI, CLI automation, and web tools should all build on these
//! package types rather than inventing their own project model.
//!
//! # Example
//!
//! ```rust
//! use trem_project::{ProjectPackage, SceneDocument};
//!
//! let package = ProjectPackage {
//!     root: std::path::PathBuf::from("easybeat"),
//!     manifest: trem_project::ProjectManifest::easybeat(),
//! };
//! assert_eq!(package.manifest.project.root_scene, "easybeat");
//! assert_eq!(package.manifest.project.tempo_bpm, 146);
//! assert_eq!(SceneDocument::easybeat().lanes.len(), 6);
//! ```

pub mod clip;
pub mod graph;
pub mod layout;
pub mod manifest;
pub mod package;
pub mod scene;

pub use clip::{ClipDocument, ClipKind, ClipMeta, ClipNote};
pub use graph::{GraphDocument, GraphMeta, GraphNodeKind, GraphNodeSpec};
pub use layout::{CLIPS_DIR, GRAPHS_DIR, MANIFEST_FILE, MIDIS_DIR, SAMPLES_DIR, SCENES_DIR};
pub use manifest::{ManifestRefs, PortKind, PortSpec, ProjectManifest, ProjectMeta};
pub use package::{PackageError, ProjectPackage};
pub use scene::{
    BlockContent, BlockSpec, LaneKind, LanePerformer, LaneSpec, SceneDocument, SceneMeta,
};
