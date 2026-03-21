//! Evenly spaced onsets (Euclidean rhythms) and cyclic pattern rotation.
//!
//! See Toussaint’s treatment of `B(k, n)` patterns; [`euclidean`] fixes the first hit on step 0.

/// Euclidean rhythm generator.
///
/// Distributes `hits` as evenly as possible across `steps`,
/// producing the family of rhythms studied by Toussaint (2005).
/// The first hit always lands on step 0.
pub fn euclidean(hits: u32, steps: u32) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }
    if hits == 0 {
        return vec![false; steps as usize];
    }
    if hits >= steps {
        return vec![true; steps as usize];
    }
    let mut pattern = vec![false; steps as usize];
    for i in 0..hits {
        let pos = (i as u64 * steps as u64 / hits as u64) as usize;
        pattern[pos] = true;
    }
    pattern
}

/// Rotate a pattern by `offset` steps to the right.
pub fn rotate(pattern: &[bool], offset: u32) -> Vec<bool> {
    let n = pattern.len();
    if n == 0 {
        return vec![];
    }
    let off = n - (offset as usize % n);
    pattern[off..]
        .iter()
        .chain(pattern[..off].iter())
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tresillo_3_8() {
        let p = euclidean(3, 8);
        assert_eq!(p.iter().filter(|&&v| v).count(), 3);
        assert_eq!(p.len(), 8);
    }

    #[test]
    fn four_on_the_floor() {
        let p = euclidean(4, 16);
        assert_eq!(p.iter().filter(|&&v| v).count(), 4);
        assert!(p[0] && p[4] && p[8] && p[12]);
    }

    #[test]
    fn five_of_eight() {
        let p = euclidean(5, 8);
        assert_eq!(p.iter().filter(|&&v| v).count(), 5);
    }

    #[test]
    fn edge_cases() {
        assert_eq!(euclidean(0, 8), vec![false; 8]);
        assert_eq!(euclidean(8, 8), vec![true; 8]);
        assert!(euclidean(5, 0).is_empty());
    }

    #[test]
    fn rotate_tresillo() {
        let p = euclidean(3, 8);
        let r = rotate(&p, 1);
        assert_eq!(r.len(), 8);
        assert_eq!(r.iter().filter(|&&v| v).count(), 3);
        assert_eq!(r[0], p[p.len() - 1]);
    }
}
