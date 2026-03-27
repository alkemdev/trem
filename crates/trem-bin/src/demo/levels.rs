//! Single source of truth for the default **demo** mix: channel trims, bus gains, and master FX.
//! Tweak here to rebalance the whole patch without hunting literals in `graph.rs`.

/// Audio block size (must match nested instrument graphs).
pub const BLOCK_SIZE: usize = 512;

/// Channel strips (mono source → stereo out).
pub mod channel {
    pub const LEAD_LEVEL: f32 = 0.48;
    pub const LEAD_PAN: f32 = 0.08;
    pub const BASS_LEVEL: f32 = 0.09;
    pub const KICK_LEVEL: f32 = 0.52;
    pub const SNARE_LEVEL: f32 = 0.28;
    pub const HAT_LEVEL: f32 = 0.12;
    pub const HAT_PAN: f32 = 0.25;
    pub const SNARE_PAN: f32 = -0.05;
}

/// Lead-only flutter delay (nested in lead channel).
pub mod lead_delay {
    pub const MS: f64 = 38.0;
    pub const FEEDBACK: f64 = 0.40;
    pub const MIX: f64 = 0.22;
}

/// Drum subgroup before master.
pub mod drum_bus {
    pub const LIMITER_CEILING_DB: f64 = -1.0;
    pub const LIMITER_RELEASE_MS: f64 = 42.0;
    pub const OUTPUT_GAIN: f32 = 0.78;
}

/// Instrument subgroup (lead + bass).
pub mod inst_bus {
    pub const COMP_THRESHOLD_DB: f64 = -18.0;
    pub const COMP_RATIO: f64 = 3.0;
    pub const COMP_ATTACK_MS: f64 = 8.0;
    pub const COMP_RELEASE_MS: f64 = 120.0;
    pub const OUTPUT_GAIN: f32 = 0.74;
}

/// Master chain: EQ → delay → reverb → limiter → out.
pub mod main_bus {
    pub const EQ_LOW_DB: f64 = -1.5;
    pub const EQ_MID_DB: f64 = 3.0;
    pub const EQ_HI_DB: f64 = 1.5;
    pub const DELAY_MS: f64 = 260.0;
    pub const DELAY_FB: f64 = 0.16;
    pub const DELAY_MIX: f64 = 0.035;
    pub const REVERB_SIZE: f64 = 0.32;
    pub const REVERB_DAMP: f64 = 0.62;
    pub const REVERB_MIX: f64 = 0.045;
    pub const LIMITER_CEILING_DB: f64 = -0.3;
    pub const LIMITER_RELEASE_MS: f64 = 100.0;
    pub const OUTPUT_GAIN: f32 = 0.82;
}
