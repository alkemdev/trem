//! Default 32×5 step pattern: pentatonic lead arp, bass cells, kick/snare/hats.

use trem::event::NoteEvent;
use trem::grid::Grid;
use trem::math::Rational;

const STEPS: u32 = 32;

/// Demo grid shipped with the binary (major pentatonic contour at ~146 BPM).
pub fn build_pattern() -> Grid {
    let mut grid = Grid::new(STEPS, 5);

    let n = |deg: i32, oct: i32, vel_n: i64, vel_d: u64| {
        NoteEvent::new(deg, oct, Rational::new(vel_n, vel_d))
    };
    let ng = |deg: i32, oct: i32, vel_n: i64, vel_d: u64, g_n: i64, g_d: u64| -> NoteEvent {
        NoteEvent::new(deg, oct, Rational::new(vel_n, vel_d)).with_gate(Rational::new(g_n, g_d))
    };

    let gate_arp = Rational::new(1, 4);
    let vm = Rational::new(11, 20);
    let vh = Rational::new(7, 10);
    let vl = Rational::new(9, 25);
    let arp = |deg, oct, vel: Rational| NoteEvent::new(deg, oct, vel).with_gate(gate_arp);

    let lead = [
        (0, arp(0, -1, vh)),
        (1, arp(2, -1, vm)),
        (2, arp(4, -1, vm)),
        (3, arp(7, -1, vm)),
        (4, arp(9, -1, vm)),
        (5, arp(7, -1, vm)),
        (6, arp(4, -1, vm)),
        (7, arp(2, -1, vl)),
        (8, arp(0, -1, vh)),
        (9, arp(0, -1, vl)),
        (10, arp(4, -1, vm)),
        (11, arp(7, -1, vm)),
        (12, arp(9, -1, vm)),
        (13, arp(7, -1, vm)),
        (14, arp(4, -1, vm)),
        (15, arp(0, 0, vh)),
        (16, arp(2, 0, vm)),
        (17, arp(4, 0, vm)),
        (18, arp(7, 0, vh)),
        (19, arp(9, 0, vm)),
        (20, arp(7, 0, vm)),
        (21, arp(4, 0, vm)),
        (22, arp(2, 0, vm)),
        (23, arp(0, 0, vm)),
        (24, arp(9, -1, vm)),
        (25, arp(7, -1, vm)),
        (26, arp(4, -1, vm)),
        (27, arp(2, -1, vm)),
        (28, arp(0, -1, vm)),
        (29, arp(7, -1, vl)),
        (30, arp(9, -1, vm)),
        (31, arp(0, -1, vh)),
    ];
    for (row, ev) in lead {
        grid.set(row, 0, Some(ev));
    }

    let bass = [
        (0, ng(0, -3, 7, 8, 7, 8)),
        (4, ng(0, -3, 5, 8, 5, 8)),
        (8, ng(5, -3, 6, 8, 6, 8)),
        (12, ng(3, -3, 5, 8, 5, 8)),
        (16, ng(0, -3, 6, 8, 7, 8)),
        (20, ng(7, -3, 5, 8, 4, 8)),
        (24, ng(5, -3, 6, 8, 5, 8)),
        (28, ng(3, -3, 5, 8, 4, 8)),
        (31, ng(10, -3, 7, 8, 3, 4)),
    ];
    for (row, ev) in bass {
        grid.set(row, 1, Some(ev));
    }

    for step in [0, 4, 8, 12, 16, 20, 24, 28, 10, 22] {
        let vel = match step {
            0 | 8 | 16 | 24 => Rational::new(29, 32),
            4 | 12 | 20 | 28 => Rational::new(5, 8),
            _ => Rational::new(3, 5),
        };
        grid.set(step, 2, Some(NoteEvent::new(0, 0, vel)));
    }

    let snare = [
        (4, 2, 5),
        (6, 1, 5),
        (8, 3, 4),
        (10, 1, 5),
        (12, 2, 5),
        (14, 1, 6),
        (20, 2, 5),
        (22, 1, 5),
        (24, 3, 4),
        (26, 1, 5),
        (28, 2, 5),
        (30, 1, 6),
    ];
    for (row, vn, vd) in snare {
        grid.set(row, 3, Some(n(0, 0, vn, vd)));
    }

    for step in 0..STEPS {
        let half = step >= 16;
        let is_downbeat = step % 8 == 0;
        let is_quarter = step % 4 == 0;
        let vel = if is_downbeat {
            if half {
                Rational::new(8, 15)
            } else {
                Rational::new(7, 20)
            }
        } else if is_quarter {
            Rational::new(11, 40)
        } else if step % 2 == 1 {
            Rational::new(1, 8)
        } else {
            Rational::new(1, 6)
        };
        let mut ev = NoteEvent::new(0, 0, vel).with_gate(Rational::new(1, 4));
        if half && is_downbeat {
            ev = ev.with_gate(Rational::new(3, 10));
        }
        grid.set(step, 4, Some(ev));
    }

    grid
}
