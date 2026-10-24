//! `cargo run --release -p kagra-shared --example runner_measure -- WORLD.json [RUNS] [--no-perf] [--feel PRESET]`
//!
//! Prints the measurement report (see `neon_cat::measure`) as JSON.
use kagra_shared::neon_cat::measure::{measure, MeasureOpts};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .expect("usage: runner_measure WORLD.json [RUNS] [--no-perf]");
    let mut world = std::fs::read_to_string(path).expect("read world");
    if let Some(i) = args.iter().position(|a| a == "--feel") {
        let name = args.get(i + 1).expect("--feel needs a preset name");
        let mut v: serde_json::Value = serde_json::from_str(&world).expect("world json");
        v["game"]["feel"] = serde_json::json!({ "preset": name });
        world = v.to_string();
    }
    let mut opts = MeasureOpts::default();
    if let Some(n) = args.get(1).and_then(|s| s.parse().ok()) {
        opts.runs = n;
    }
    opts.perf = !args.iter().any(|a| a == "--no-perf");
    let report = measure(&world, &opts).unwrap_or_else(|e| panic!("{e}"));
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
