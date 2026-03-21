//! White noise from a small deterministic PRNG—useful for percussion, air, or modulation sources.

use crate::graph::{ProcessContext, Processor, ProcessorInfo, Sig};

/// White noise generator using a linear congruential generator.
/// No external dependencies — deterministic PRNG seeded at construction.
pub struct Noise {
    state: u32,
}

impl Noise {
    /// Default seed; same instance always produces the same sequence after construction or [`Processor::reset`].
    pub fn new() -> Self {
        Self { state: 0x12345678 }
    }

    /// Chooses a starting LCG state (forced odd) so parallel noise nodes can be uncorrelated.
    pub fn with_seed(seed: u32) -> Self {
        Self { state: seed | 1 }
    }

    fn next_sample(&mut self) -> f32 {
        // LCG: state = state * 1664525 + 1013904223 (Numerical Recipes)
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        // Map u32 to [-1, 1]
        (self.state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for Noise {
    fn info(&self) -> ProcessorInfo {
        ProcessorInfo {
            name: "noise",
            sig: Sig::SOURCE1,
            description: "White noise generator",
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        for i in 0..ctx.frames {
            ctx.outputs[0][i] = self.next_sample();
        }
    }

    fn reset(&mut self) {
        self.state = 0x12345678;
    }
}
