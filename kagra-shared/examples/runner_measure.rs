//! `cargo run --release -p kagra-shared --example runner_measure -- WORLD.json [RUNS] [--no-perf]`
//!
//! Prints the measurement report (see `neon_cat::measure`) as JSON.
use kagra_shared::neon_cat::measure::{measure, MeasureOpts};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .expect("usage: runner_measure WORLD.json [RUNS] [--no-perf]");
    let world = std::fs::read_to_string(path).expect("read world");
    let mut opts = MeasureOpts::default();
    if let Some(n) = args.get(1).and_then(|s| s.parse().ok()) {
        opts.runs = n;
    }
    opts.perf = !args.iter().any(|a| a == "--no-perf");
    let report = measure(&world, &opts).unwrap_or_else(|e| panic!("{e}"));
    if args.iter().any(|a| a == "--check") {
        // Placement validator only: print issues, exit 1 on any error.
        let p = &report["course"]["placement"];
        println!("{}", serde_json::to_string_pretty(p).unwrap());
        if p["ok"] != true {
            std::process::exit(1);
        }
        return;
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
