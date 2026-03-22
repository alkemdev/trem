//! Euclidean rhythm generation: distribute hits evenly across steps.
//!
//! Run with: `cargo run -p trem --example euclidean_rhythm`

use trem::euclidean;

fn print_pattern(name: &str, hits: u32, steps: u32) {
    let p = euclidean::euclidean(hits, steps);
    let display: String = p.iter().map(|&b| if b { 'x' } else { '.' }).collect();
    println!("E({hits},{steps})  {display:>16}  {name}");
}

fn main() {
    println!("Euclidean rhythms — E(hits, steps)\n");

    print_pattern("tresillo", 3, 8);
    print_pattern("four on the floor", 4, 16);
    print_pattern("cinquillo", 5, 8);
    print_pattern("bembé", 7, 12);
    print_pattern("aksak", 5, 9);
    print_pattern("son clave", 5, 16);

    println!("\nRotation example:");
    let base = euclidean::euclidean(3, 8);
    for offset in 0..4 {
        let r = euclidean::rotate(&base, offset);
        let display: String = r.iter().map(|&b| if b { 'x' } else { '.' }).collect();
        println!("  rot {offset}: {display}");
    }
}
