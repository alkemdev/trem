use crate::event::NoteEvent;
use crate::tree::Tree;

/// A Grid is a 2D (rows x columns) editable view of a pattern tree.
///
/// Rows are time divisions, columns are polyphony voices.
/// The grid owns its data as a flat vec and can rebuild a Tree on demand.
#[derive(Clone, Debug)]
pub struct Grid {
    /// Number of time rows.
    pub rows: u32,
    /// Number of voice columns.
    pub columns: u32,
    /// Flat cell storage, indexed by row * columns + col.
    pub cells: Vec<Option<NoteEvent>>,
}

impl Grid {
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

    pub fn get(&self, row: u32, col: u32) -> Option<&NoteEvent> {
        self.cells[self.idx(row, col)].as_ref()
    }

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

    /// Count non-empty cells.
    pub fn count_events(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Check if a row has any events.
    pub fn row_has_events(&self, row: u32) -> bool {
        (0..self.columns).any(|col| self.get(row, col).is_some())
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
