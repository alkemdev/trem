use crate::math::Rational;

/// Recursive temporal tree structure.
///
/// A Tree subdivides a time span. Children of `Seq` divide the parent's span
/// evenly (or by explicit weights). Children of `Par` overlap for the full
/// parent duration. This encodes arbitrary rhythmic subdivision with zero
/// special cases.
#[derive(Clone, Debug, PartialEq)]
pub enum Tree<E> {
    /// A leaf event spanning the parent's full duration.
    Leaf(E),
    /// Silence for the parent's full duration.
    Rest,
    /// Sequential subdivision — children divide time evenly.
    Seq(Vec<Tree<E>>),
    /// Parallel overlap — all children span the full duration.
    Par(Vec<Tree<E>>),
    /// Weighted sequential — children have explicit relative durations.
    /// Weights are ratios; actual durations are weight/total_weight * parent_duration.
    Weight(Vec<(Rational, Tree<E>)>),
}

/// A flattened event with its position and duration relative to [0, 1).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatEvent<'a, E> {
    pub start: Rational,
    pub duration: Rational,
    pub event: &'a E,
}

/// Owned version for cases where we need to collect across borrows.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedFlatEvent<E> {
    pub start: Rational,
    pub duration: Rational,
    pub event: E,
}

impl<E> Tree<E> {
    pub fn leaf(event: E) -> Self {
        Tree::Leaf(event)
    }

    pub fn rest() -> Self {
        Tree::Rest
    }

    pub fn seq(children: Vec<Tree<E>>) -> Self {
        Tree::Seq(children)
    }

    pub fn par(children: Vec<Tree<E>>) -> Self {
        Tree::Par(children)
    }

    pub fn weight(children: Vec<(Rational, Tree<E>)>) -> Self {
        Tree::Weight(children)
    }

    /// Flatten the tree to timed event references.
    /// Times are in [0, 1) relative to the tree's total span.
    pub fn flatten(&self) -> Vec<FlatEvent<'_, E>> {
        let mut out = Vec::new();
        self.flatten_into(Rational::zero(), Rational::one(), &mut out);
        out
    }

    fn flatten_into<'a>(
        &'a self,
        offset: Rational,
        span: Rational,
        out: &mut Vec<FlatEvent<'a, E>>,
    ) {
        match self {
            Tree::Leaf(e) => {
                out.push(FlatEvent {
                    start: offset,
                    duration: span,
                    event: e,
                });
            }
            Tree::Rest => {}
            Tree::Seq(children) => {
                if children.is_empty() {
                    return;
                }
                let n = children.len() as u64;
                let child_span = span * Rational::new(1, n);
                for (i, child) in children.iter().enumerate() {
                    let child_offset = offset + child_span * Rational::integer(i as i64);
                    child.flatten_into(child_offset, child_span, out);
                }
            }
            Tree::Par(children) => {
                for child in children {
                    child.flatten_into(offset, span, out);
                }
            }
            Tree::Weight(children) => {
                let total: Rational = children
                    .iter()
                    .map(|(w, _)| *w)
                    .fold(Rational::zero(), |a, b| a + b);
                if total.is_zero() {
                    return;
                }
                let mut cursor = offset;
                for (w, child) in children {
                    let child_span = span * *w / total;
                    child.flatten_into(cursor, child_span, out);
                    cursor = cursor + child_span;
                }
            }
        }
    }

    /// Query events whose time range intersects [start, end).
    /// Times are relative to the tree's [0, 1) span.
    pub fn query(&self, start: Rational, end: Rational) -> Vec<FlatEvent<'_, E>> {
        self.flatten()
            .into_iter()
            .filter(|fe| fe.start < end && fe.start + fe.duration > start)
            .collect()
    }

    /// Map a function over all leaf events.
    pub fn map<F, U>(self, f: &F) -> Tree<U>
    where
        F: Fn(E) -> U,
    {
        match self {
            Tree::Leaf(e) => Tree::Leaf(f(e)),
            Tree::Rest => Tree::Rest,
            Tree::Seq(children) => Tree::Seq(children.into_iter().map(|c| c.map(f)).collect()),
            Tree::Par(children) => Tree::Par(children.into_iter().map(|c| c.map(f)).collect()),
            Tree::Weight(children) => {
                Tree::Weight(children.into_iter().map(|(w, c)| (w, c.map(f))).collect())
            }
        }
    }

    /// Fold over all leaf events, depth-first.
    pub fn fold<A, F>(&self, init: A, f: &F) -> A
    where
        F: Fn(A, &E) -> A,
    {
        match self {
            Tree::Leaf(e) => f(init, e),
            Tree::Rest => init,
            Tree::Seq(children) | Tree::Par(children) => {
                children.iter().fold(init, |acc, c| c.fold(acc, f))
            }
            Tree::Weight(children) => children.iter().fold(init, |acc, (_, c)| c.fold(acc, f)),
        }
    }

    /// Count the leaf events in the tree.
    pub fn count_leaves(&self) -> usize {
        self.fold(0, &|acc, _| acc + 1)
    }

    /// Maximum nesting depth.
    pub fn depth(&self) -> usize {
        match self {
            Tree::Leaf(_) | Tree::Rest => 0,
            Tree::Seq(c) | Tree::Par(c) => c.iter().map(|ch| ch.depth()).max().unwrap_or(0) + 1,
            Tree::Weight(c) => c.iter().map(|(_, ch)| ch.depth()).max().unwrap_or(0) + 1,
        }
    }
}

impl<E: Clone> Tree<E> {
    /// Flatten to owned events (clones leaf data).
    pub fn flatten_owned(&self) -> Vec<OwnedFlatEvent<E>> {
        self.flatten()
            .into_iter()
            .map(|fe| OwnedFlatEvent {
                start: fe.start,
                duration: fe.duration,
                event: fe.event.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_single_leaf() {
        let tree = Tree::leaf(42);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].start, Rational::zero());
        assert_eq!(flat[0].duration, Rational::one());
        assert_eq!(*flat[0].event, 42);
    }

    #[test]
    fn flat_seq_4() {
        let tree = Tree::seq(vec![
            Tree::leaf(0),
            Tree::leaf(1),
            Tree::leaf(2),
            Tree::leaf(3),
        ]);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 4);
        assert_eq!(flat[0].start, Rational::zero());
        assert_eq!(flat[0].duration, Rational::new(1, 4));
        assert_eq!(flat[3].start, Rational::new(3, 4));
    }

    #[test]
    fn flat_nested_triplet() {
        // 4 beats, third beat is a triplet
        let tree = Tree::seq(vec![
            Tree::leaf("a"),
            Tree::leaf("b"),
            Tree::seq(vec![Tree::leaf("c"), Tree::leaf("d"), Tree::leaf("e")]),
            Tree::leaf("f"),
        ]);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 6);
        // The triplet events should each be 1/12 of the total span
        assert_eq!(flat[2].duration, Rational::new(1, 12));
        assert_eq!(flat[3].duration, Rational::new(1, 12));
        assert_eq!(flat[4].duration, Rational::new(1, 12));
    }

    #[test]
    fn parallel_overlap() {
        let tree = Tree::par(vec![Tree::leaf("bass"), Tree::leaf("melody")]);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 2);
        // Both span the full duration
        assert_eq!(flat[0].duration, Rational::one());
        assert_eq!(flat[1].duration, Rational::one());
        assert_eq!(flat[0].start, flat[1].start);
    }

    #[test]
    fn weighted_seq() {
        // 3:1 split — first child gets 3/4, second gets 1/4
        let tree = Tree::weight(vec![
            (Rational::integer(3), Tree::leaf("long")),
            (Rational::one(), Tree::leaf("short")),
        ]);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].duration, Rational::new(3, 4));
        assert_eq!(flat[1].duration, Rational::new(1, 4));
        assert_eq!(flat[1].start, Rational::new(3, 4));
    }

    #[test]
    fn rest_produces_no_events() {
        let tree: Tree<i32> = Tree::seq(vec![Tree::leaf(1), Tree::rest(), Tree::leaf(3)]);
        let flat = tree.flatten();
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn query_range() {
        let tree = Tree::seq(vec![
            Tree::leaf(0),
            Tree::leaf(1),
            Tree::leaf(2),
            Tree::leaf(3),
        ]);
        let hits = tree.query(Rational::new(1, 4), Rational::new(3, 4));
        assert_eq!(hits.len(), 2);
        assert_eq!(*hits[0].event, 1);
        assert_eq!(*hits[1].event, 2);
    }

    #[test]
    fn map_and_fold() {
        let tree = Tree::seq(vec![Tree::leaf(1), Tree::leaf(2), Tree::leaf(3)]);
        let doubled = tree.map(&|x| x * 2);
        let sum = doubled.fold(0, &|acc, x| acc + x);
        assert_eq!(sum, 12);
    }

    #[test]
    fn depth_nested() {
        let tree = Tree::seq(vec![
            Tree::leaf(0),
            Tree::seq(vec![Tree::leaf(1), Tree::seq(vec![Tree::leaf(2)])]),
        ]);
        assert_eq!(tree.depth(), 3);
    }
}
