//! Authored graph documents stored under `graphs/*.json`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One graph document authored inside a project package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphDocument {
    /// Graph identity and routing role.
    pub graph: GraphMeta,
    /// Nodes in this graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<GraphNodeSpec>,
}

/// Metadata for a graph document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphMeta {
    /// Stable graph id within the package.
    pub id: String,
    /// Human-visible graph label.
    pub name: String,
}

/// One node in an authored graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNodeSpec {
    /// Stable node id within the graph.
    pub id: String,
    /// Human-visible label.
    pub label: String,
    /// Concrete node source.
    #[serde(flatten)]
    pub kind: GraphNodeKind,
    /// Input node ids for a simple authored graph view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// Named parameter values.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f64>,
}

/// Backing implementation for an authored node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNodeKind {
    Input { port: String },
    Output { port: String },
    Standard { tag: String },
}

impl GraphDocument {
    /// Structural validation for node ids and references.
    pub fn validate_basic(&self) -> Result<(), String> {
        if self.graph.id.trim().is_empty() {
            return Err("graph.id must not be empty".into());
        }
        if self.graph.name.trim().is_empty() {
            return Err("graph.name must not be empty".into());
        }
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            if !ids.insert(node.id.as_str()) {
                return Err(format!("duplicate node id {:?}", node.id));
            }
            if node.label.trim().is_empty() {
                return Err(format!("node {:?} has an empty label", node.id));
            }
            match &node.kind {
                GraphNodeKind::Input { port } | GraphNodeKind::Output { port } => {
                    if port.trim().is_empty() {
                        return Err(format!("node {:?} has an empty port", node.id));
                    }
                }
                GraphNodeKind::Standard { tag } => {
                    if tag.trim().is_empty() {
                        return Err(format!("node {:?} has an empty standard tag", node.id));
                    }
                }
            }
        }
        for node in &self.nodes {
            for input in &node.inputs {
                if !ids.contains(input.as_str()) {
                    return Err(format!(
                        "node {:?} references missing input node {:?}",
                        node.id, input
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_validation_rejects_missing_inputs() {
        let graph = GraphDocument {
            graph: GraphMeta {
                id: "lead".into(),
                name: "Lead".into(),
            },
            nodes: vec![GraphNodeSpec {
                id: "delay".into(),
                label: "Delay".into(),
                kind: GraphNodeKind::Standard { tag: "dly".into() },
                inputs: vec!["voice".into()],
                params: BTreeMap::new(),
            }],
        };
        let err = graph.validate_basic().expect_err("missing input");
        assert!(err.contains("missing input"));
    }
}
