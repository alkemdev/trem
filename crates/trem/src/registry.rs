//! Tag registry: create [`Node`] instances by short mnemonic at runtime.
//!
//! The [`Registry`] maps 3-letter tags (e.g. `"osc"`, `"dly"`, `"vrb"`) to factories
//! that build boxed [`Node`] trait objects. Stock implementations and `register_standard`
//! live in the **`trem-dsp`** crate.

use crate::graph::Node;
use std::collections::HashMap;

/// Category classification for grouping nodes in UIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Sound generators: oscillators, noise, drum voices, composite synths.
    Source,
    /// Time-based effects: delay, reverb, EQ.
    Effect,
    /// Summing and crossfade stages.
    Mixer,
    /// Compressors, limiters, gates.
    Dynamics,
    /// Frequency-domain nodes: biquad, graphic EQ.
    Filter,
    /// Control-rate generators: envelopes, LFOs.
    Modulator,
    /// Gain, panning, and other routing utilities.
    Utility,
}

impl Category {
    /// Human-readable name for display in menus and headings.
    pub fn label(self) -> &'static str {
        match self {
            Category::Source => "Source",
            Category::Effect => "Effect",
            Category::Mixer => "Mixer",
            Category::Dynamics => "Dynamics",
            Category::Filter => "Filter",
            Category::Modulator => "Modulator",
            Category::Utility => "Utility",
        }
    }
}

/// Metadata and factory for one registered tag ([`Registry::register`]).
pub struct NodeEntry {
    /// Short mnemonic used as the lookup key (e.g. `"osc"`, `"dly"`).
    pub tag: &'static str,
    /// Full human-readable name (e.g. `"Oscillator"`, `"Stereo Delay"`).
    pub name: &'static str,
    /// Grouping category for UI display.
    pub category: Category,
    factory: Box<dyn Fn() -> Box<dyn Node> + Send + Sync>,
}

impl NodeEntry {
    /// Instantiate a new default node from this entry's factory.
    pub fn create(&self) -> Box<dyn Node> {
        (self.factory)()
    }
}

/// Runtime mapping from tags to [`Node`] factories.
///
/// Use **`trem-dsp`** (`register_standard`, `standard_registry`) for all built-in tags,
/// or [`Registry::new`] + [`Registry::register`] for a custom set.
pub struct Registry {
    entries: HashMap<&'static str, NodeEntry>,
    by_category: HashMap<Category, Vec<&'static str>>,
}

impl Registry {
    /// Empty registry with no entries.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a [`Node`] factory under a tag.
    pub fn register<F>(&mut self, tag: &'static str, name: &'static str, category: Category, f: F)
    where
        F: Fn() -> Box<dyn Node> + Send + Sync + 'static,
    {
        self.by_category.entry(category).or_default().push(tag);
        self.entries.insert(
            tag,
            NodeEntry {
                tag,
                name,
                category,
                factory: Box::new(f),
            },
        );
    }

    /// Create a [`Node`] by tag. Returns `None` if the tag is unknown.
    pub fn create(&self, tag: &str) -> Option<Box<dyn Node>> {
        self.entries.get(tag).map(|e| e.create())
    }

    /// Look up entry metadata by tag.
    pub fn get(&self, tag: &str) -> Option<&NodeEntry> {
        self.entries.get(tag)
    }

    /// All registered tags.
    pub fn tags(&self) -> Vec<&'static str> {
        let mut tags: Vec<_> = self.entries.keys().copied().collect();
        tags.sort();
        tags
    }

    /// Tags belonging to a category.
    pub fn tags_in(&self, category: Category) -> &[&'static str] {
        self.by_category
            .get(&category)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// All distinct categories that have at least one entry.
    pub fn categories(&self) -> Vec<Category> {
        let mut cats: Vec<_> = self.by_category.keys().copied().collect();
        cats.sort_by_key(|c| c.label());
        cats
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Node, NodeInfo, ProcessContext, Sig};

    struct Passthrough;

    impl Node for Passthrough {
        fn info(&self) -> NodeInfo {
            NodeInfo {
                name: "passthrough",
                sig: Sig::MONO,
                description: "test",
            }
        }

        fn process(&mut self, ctx: &mut ProcessContext) {
            for i in 0..ctx.frames {
                ctx.outputs[0][i] = ctx.inputs[0][i];
            }
        }

        fn reset(&mut self) {}
    }

    #[test]
    fn new_registry_is_empty() {
        let reg = Registry::new();
        assert!(reg.tags().is_empty());
        assert!(reg.create("osc").is_none());
    }

    #[test]
    fn register_and_create() {
        let mut reg = Registry::new();
        reg.register("tst", "Test", Category::Utility, || Box::new(Passthrough));
        let p = reg.create("tst").unwrap();
        assert_eq!(p.info().name, "passthrough");
    }
}
