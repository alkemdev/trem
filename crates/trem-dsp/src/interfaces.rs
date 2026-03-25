//! Types from [`trem`] used to implement audio [`Node`] nodes.
//!
//! Third-party crates can depend on `trem` directly or use this module for one import path
//! alongside [`crate::standard`] stock implementations.

pub use trem::event::{GraphEvent, TimedEvent};
pub use trem::graph::{
    Graph, GraphInput, GroupHint, Node, NodeId, NodeInfo, ParamDescriptor, ParamFlags, ParamGroup,
    ParamUnit, PortIdx, PrepareEnv, PrepareError, ProcessContext, Sig,
};
