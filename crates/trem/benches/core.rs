use divan::Bencher;
use trem::euclidean;
use trem::event::NoteEvent;
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::{Pitch, Tuning};
use trem::tree::Tree;

fn main() {
    divan::main();
}

// ---------------------------------------------------------------------------
// Rational arithmetic
// ---------------------------------------------------------------------------

mod rational {
    use super::*;

    #[divan::bench]
    fn new_reduce(bencher: Bencher) {
        bencher.bench(|| Rational::new(355, 113));
    }

    #[divan::bench]
    fn add(bencher: Bencher) {
        let a = Rational::new(1, 3);
        let b = Rational::new(1, 4);
        bencher.bench(|| a + b);
    }

    #[divan::bench]
    fn mul(bencher: Bencher) {
        let a = Rational::new(7, 12);
        let b = Rational::new(3, 8);
        bencher.bench(|| a * b);
    }

    #[divan::bench]
    fn div(bencher: Bencher) {
        let a = Rational::new(7, 12);
        let b = Rational::new(3, 8);
        bencher.bench(|| a / b);
    }

    #[divan::bench]
    fn floor(bencher: Bencher) {
        let r = Rational::new(7, 3);
        bencher.bench(|| r.floor());
    }

    #[divan::bench]
    fn to_f64(bencher: Bencher) {
        let r = Rational::new(355, 113);
        bencher.bench(|| r.to_f64());
    }

    #[divan::bench]
    fn cmp(bencher: Bencher) {
        let a = Rational::new(355, 113);
        let b = Rational::new(22, 7);
        bencher.bench(|| a < b);
    }
}

// ---------------------------------------------------------------------------
// Tree operations
// ---------------------------------------------------------------------------

mod tree {
    use super::*;

    fn make_deep_tree(depth: u32) -> Tree<i32> {
        if depth == 0 {
            Tree::leaf(1)
        } else {
            Tree::seq(vec![
                make_deep_tree(depth - 1),
                Tree::rest(),
                make_deep_tree(depth - 1),
                Tree::leaf(depth as i32),
            ])
        }
    }

    #[divan::bench(args = [4, 8, 16])]
    fn flatten_seq(bencher: Bencher, n: usize) {
        let tree = Tree::seq((0..n).map(Tree::leaf).collect());
        bencher.bench(|| tree.flatten());
    }

    #[divan::bench(args = [3, 5, 7])]
    fn flatten_deep(bencher: Bencher, depth: u32) {
        let tree = make_deep_tree(depth);
        bencher.bench(|| tree.flatten());
    }

    #[divan::bench]
    fn flatten_nested_triplet() {
        let tree = Tree::seq(vec![
            Tree::leaf(0),
            Tree::leaf(1),
            Tree::seq(vec![Tree::leaf(2), Tree::leaf(3), Tree::leaf(4)]),
            Tree::leaf(5),
        ]);
        divan::black_box(tree.flatten());
    }

    #[divan::bench]
    fn weighted_flatten(bencher: Bencher) {
        let tree = Tree::weight(vec![
            (Rational::integer(3), Tree::leaf("long")),
            (Rational::one(), Tree::leaf("short")),
            (Rational::integer(2), Tree::leaf("medium")),
        ]);
        bencher.bench(|| tree.flatten());
    }

    #[divan::bench]
    fn query_range(bencher: Bencher) {
        let tree = Tree::seq((0..16).map(Tree::leaf).collect());
        bencher.bench(|| tree.query(Rational::new(1, 4), Rational::new(3, 4)));
    }

    #[divan::bench]
    fn count_leaves(bencher: Bencher) {
        let tree = make_deep_tree(6);
        bencher.bench(|| tree.count_leaves());
    }
}

// ---------------------------------------------------------------------------
// Grid operations
// ---------------------------------------------------------------------------

mod grid {
    use super::*;

    fn populated_grid(rows: u32, cols: u32) -> Grid {
        let mut g = Grid::new(rows, cols);
        for r in (0..rows).step_by(2) {
            for c in 0..cols {
                g.set(r, c, Some(NoteEvent::simple(r as i32 % 7)));
            }
        }
        g
    }

    #[divan::bench(args = [16, 32, 64])]
    fn to_tree(bencher: Bencher, rows: u32) {
        let grid = populated_grid(rows, 5);
        bencher.bench(|| grid.to_tree());
    }

    #[divan::bench]
    fn from_tree(bencher: Bencher) {
        let grid = populated_grid(16, 5);
        let tree = grid.to_tree();
        bencher.bench(|| Grid::from_tree(&tree, 16, 5));
    }

    #[divan::bench]
    fn column_tree(bencher: Bencher) {
        let grid = populated_grid(32, 5);
        bencher.bench(|| grid.column_tree(0));
    }

    #[divan::bench]
    fn shift_voice(bencher: Bencher) {
        let mut grid = populated_grid(16, 5);
        bencher.bench_local(|| grid.shift_voice(0, 3));
    }

    #[divan::bench]
    fn reverse_voice(bencher: Bencher) {
        let mut grid = populated_grid(16, 5);
        bencher.bench_local(|| grid.reverse_voice(0));
    }

    #[divan::bench]
    fn fill_euclidean(bencher: Bencher) {
        let mut grid = Grid::new(16, 1);
        let pattern = euclidean::euclidean(5, 16);
        let template = NoteEvent::simple(0);
        bencher.bench_local(|| grid.fill_euclidean(0, &pattern, template.clone()));
    }
}

// ---------------------------------------------------------------------------
// Euclidean rhythms
// ---------------------------------------------------------------------------

mod euclidean_bench {
    use super::*;

    #[divan::bench(args = [(3, 8), (5, 16), (7, 16), (4, 16), (11, 32)])]
    fn generate(bencher: Bencher, (k, n): (u32, u32)) {
        bencher.bench(|| euclidean::euclidean(k, n));
    }

    #[divan::bench]
    fn rotate_pattern(bencher: Bencher) {
        let p = euclidean::euclidean(5, 16);
        bencher.bench(|| euclidean::rotate(&p, 3));
    }
}

// ---------------------------------------------------------------------------
// Pitch / Scale
// ---------------------------------------------------------------------------

mod pitch {
    use super::*;

    #[divan::bench]
    fn edo12_to_scale(bencher: Bencher) {
        let tuning = Tuning::edo12();
        bencher.bench(|| tuning.to_scale());
    }

    #[divan::bench]
    fn scale_resolve(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        bencher.bench(|| scale.resolve(7));
    }

    #[divan::bench]
    fn scale_resolve_negative(bencher: Bencher) {
        let scale = Tuning::edo12().to_scale();
        bencher.bench(|| scale.resolve(-5));
    }

    #[divan::bench]
    fn pitch_to_hz(bencher: Bencher) {
        let p = Pitch::from_cents(700.0);
        bencher.bench(|| p.to_hz(440.0));
    }

    #[divan::bench]
    fn just_intonation_scale(bencher: Bencher) {
        let tuning = Tuning::Just {
            ratios: vec![
                Rational::new(1, 1),
                Rational::new(9, 8),
                Rational::new(5, 4),
                Rational::new(4, 3),
                Rational::new(3, 2),
                Rational::new(5, 3),
                Rational::new(15, 8),
            ],
        };
        bencher.bench(|| tuning.to_scale());
    }
}
