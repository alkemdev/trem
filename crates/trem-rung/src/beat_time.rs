//! Exact beat times as reduced rationals, JSON `"n/d"` or integer.

use num_rational::Rational64;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Duration or position in **beats** (quarter-note units when importing MIDI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BeatTime(pub Rational64);

impl BeatTime {
    pub fn from_int(n: i64) -> Self {
        Self(Rational64::from_integer(n))
    }

    pub fn new(num: i64, den: u64) -> Self {
        Self(Rational64::new(num, den as i64))
    }

    pub fn rational(self) -> Rational64 {
        self.0
    }
}

impl fmt::Display for BeatTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.0;
        if r.denom() == &1 {
            write!(f, "{}", r.numer())
        } else {
            write!(f, "{}/{}", r.numer(), r.denom())
        }
    }
}

impl FromStr for BeatTime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some((a, b)) = s.split_once('/') {
            let num: i64 = a
                .trim()
                .parse()
                .map_err(|e| format!("beat numerator: {e}"))?;
            let den: i64 = b
                .trim()
                .parse()
                .map_err(|e| format!("beat denominator: {e}"))?;
            if den <= 0 {
                return Err("beat denominator must be positive".into());
            }
            Ok(Self(Rational64::new(num, den)))
        } else {
            let n: i64 = s.parse().map_err(|e| format!("beat integer: {e}"))?;
            Ok(Self(Rational64::from_integer(n)))
        }
    }
}

impl Serialize for BeatTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for BeatTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = BeatTime;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string \"n/d\" or integer beat, or a number")
            }

            fn visit_str<E>(self, v: &str) -> Result<BeatTime, E>
            where
                E: serde::de::Error,
            {
                BeatTime::from_str(v).map_err(E::custom)
            }

            fn visit_i64<E>(self, v: i64) -> Result<BeatTime, E>
            where
                E: serde::de::Error,
            {
                Ok(BeatTime(Rational64::from_integer(v)))
            }

            fn visit_u64<E>(self, v: u64) -> Result<BeatTime, E>
            where
                E: serde::de::Error,
            {
                Ok(BeatTime(Rational64::from_integer(v as i64)))
            }

            fn visit_f64<E>(self, v: f64) -> Result<BeatTime, E>
            where
                E: serde::de::Error,
            {
                if !v.is_finite() {
                    return Err(E::custom("beat must be finite"));
                }
                Err(E::custom(
                    "floating beat times are not allowed in rung JSON — use \"n/d\" string",
                ))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
