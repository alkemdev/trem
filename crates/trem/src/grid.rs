//! Row/column pattern editor backed by a flat cell vec and [`Tree`] conversion.
//!
//! Rows are time steps, columns are voices; Euclidean fills and column ops target one voice at a time.

use crate::event::NoteEvent;
use crate::tree::Tree;

/// A Grid is a 2D (rows x columns) editable view of a pattern tree.
///
/// Rows are time divisions, columns are polyphony voices.
/// The grid owns its data as a flat vec and can rebuild a Tree on demand.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grid {
    /// Number of time rows.
    pub rows: u32,
    /// Number of voice columns.
    pub columns: u32,
    /// Flat cell storage, indexed by row * columns + col.
    pub cells: Vec<Option<NoteEvent>>,
}

impl Grid {
    /// Empty grid: `rows * columns` cells, all `None`.
    pub fn new(rows: u32, columns: u32) -> Self {
        let size = (rows * columns) as usize;
        Self {
            rows,
            columns,
            cells: vec![None; size],
        }
    }

    fn idx(&self, row: u32, col: u32) -> usize {
        (row * self.columns + col) as usize
    }

    /// Borrows the note at `(row, col)`, or `None` if the cell is empty. Panics if `row`/`col` are out of bounds.
    pub fn get(&self, row: u32, col: u32) -> Option<&NoteEvent> {
        self.cells[self.idx(row, col)].as_ref()
    }

    /// Writes `event` into `(row, col)`. Panics if `row`/`col` are out of bounds.
    pub fn set(&mut self, row: u32, col: u32, event: Option<NoteEvent>) {
        let idx = self.idx(row, col);
        self.cells[idx] = event;
    }

    /// Build a Tree from the grid.
    ///
    /// Each row becomes a Seq child. Columns with events at the same row
    /// become Par children. Empty rows become Rest.
    pub fn to_tree(&self) -> Tree<NoteEvent> {
        let row_trees: Vec<Tree<NoteEvent>> = (0..self.rows)
            .map(|row| {
                let events: Vec<Tree<NoteEvent>> = (0..self.columns)
                    .filter_map(|col| self.get(row, col).map(|e| Tree::leaf(e.clone())))
                    .collect();
                match events.len() {
                    0 => Tree::rest(),
                    1 => events.into_iter().next().unwrap(),
                    _ => Tree::par(events),
                }
            })
            .collect();
        Tree::seq(row_trees)
    }

    /// Populate the grid from a tree by flattening it.
    ///
    /// Events are quantized to the nearest row. If multiple events
    /// land on the same row, they fill successive columns.
    pub fn from_tree(tree: &Tree<NoteEvent>, rows: u32, columns: u32) -> Self {
        let mut grid = Self::new(rows, columns);
        let flat = tree.flatten();
        for fe in &flat {
            let row_f = fe.start.to_f64() * rows as f64;
            let row = (row_f.round() as u32).min(rows - 1);
            // Find first free column in this row
            for col in 0..columns {
                if grid.get(row, col).is_none() {
                    grid.set(row, col, Some(fe.event.clone()));
                    break;
                }
            }
        }
        grid
    }

    /// Number of cells holding a [`NoteEvent`].
    pub fn count_events(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// `true` if any column in `row` is non-empty. Panics if `row` is out of bounds.
    pub fn row_has_events(&self, row: u32) -> bool {
        (0..self.columns).any(|col| self.get(row, col).is_some())
    }

    /// Rotates column `col` vertically: positive `offset` moves events toward higher row indices, wrapping modulo `rows`.
    pub fn shift_voice(&mut self, col: u32, offset: i32) {
        let n = self.rows as i32;
        if n == 0 {
            return;
        }
        let old: Vec<Option<NoteEvent>> =
            (0..self.rows).map(|r| self.get(r, col).cloned()).collect();
        for row in 0..self.rows {
            let src = (row as i32 - offset).rem_euclid(n) as u32;
            self.set(row, col, old[src as usize].clone());
        }
    }

    /// Mirrors column `col` top-to-bottom (row `i` swaps with `rows - 1 - i`).
    pub fn reverse_voice(&mut self, col: u32) {
        let n = self.rows;
        for i in 0..n / 2 {
            let j = n - 1 - i;
            let a = self.get(i, col).cloned();
            let b = self.get(j, col).cloned();
            self.set(i, col, b);
            self.set(j, col, a);
        }
    }

    /// Sets every cell in column `col` to `None`.
    pub fn clear_voice(&mut self, col: u32) {
        for row in 0..self.rows {
            self.set(row, col, None);
        }
    }

    /// Writes `template` on rows where `pattern[i]` is true, clears others; only the first `rows` entries of `pattern` are used.
    pub fn fill_euclidean(&mut self, col: u32, pattern: &[bool], template: NoteEvent) {
        for (i, &hit) in pattern.iter().enumerate().take(self.rows as usize) {
            self.set(
                i as u32,
                col,
                if hit { Some(template.clone()) } else { None },
            );
        }
    }

    /// Extract a single column (voice) as a sequential tree.
    pub fn column_tree(&self, col: u32) -> Tree<NoteEvent> {
        let rows: Vec<Tree<NoteEvent>> = (0..self.rows)
            .map(|row| match self.get(row, col) {
                Some(e) => Tree::leaf(e.clone()),
                None => Tree::rest(),
            })
            .collect();
        Tree::seq(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Rational;

    #[test]
    fn roundtrip_simple() {
        let mut grid = Grid::new(4, 1);
        grid.set(0, 0, Some(NoteEvent::simple(0)));
        grid.set(2, 0, Some(NoteEvent::simple(4)));

        let tree = grid.to_tree();
        assert_eq!(tree.count_leaves(), 2);

        let grid2 = Grid::from_tree(&tree, 4, 1);
        assert_eq!(grid2.count_events(), 2);
        assert!(grid2.get(0, 0).is_some());
        assert!(grid2.get(2, 0).is_some());
    }

    #[test]
    fn polyphony() {
        let mut grid = Grid::new(4, 2);
        grid.set(0, 0, Some(NoteEvent::simple(0)));
        grid.set(0, 1, Some(NoteEvent::simple(4)));

        let tree = grid.to_tree();
        let flat = tree.flatten();
        // Row 0 should produce 2 events via Par
        let row0_events: Vec<_> = flat
            .iter()
            .filter(|fe| fe.start == Rational::zero())
            .collect();
        assert_eq!(row0_events.len(), 2);
    }
}
