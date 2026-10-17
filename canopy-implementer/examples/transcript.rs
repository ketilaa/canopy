//! Prints the full transcript of one or more runs: the complete prompt of every attempt, everything the
//! model said and called, every tool result and gate verdict, and how each attempt ended.
//!
//! usage: transcript <run dir or events.jsonl>...
use std::path::Path;

use canopy_implementer::transcript::render_jsonl;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: transcript <run dir or events.jsonl>...");
        std::process::exit(2);
    }
    for arg in args {
        let given = Path::new(&arg);
        let path = if given.is_dir() { given.join("events.jsonl") } else { given.to_path_buf() };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                println!("\n==================== {} ====================", path.display());
                print!("{}", render_jsonl(&text));
            }
            Err(e) => eprintln!("{}: {e}", path.display()),
        }
    }
}
