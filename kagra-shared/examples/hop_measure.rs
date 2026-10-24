//! HOP-BEAT measure report: `cargo run -p kagra-shared --example hop_measure`
//! prints it; `-- --write` refreshes docs/measure/hop_beat_baseline.json.
//! An optional path argument measures that game spec instead of the default.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--dump-world") {
        let g = kagra_shared::hop_beat::HopBeat::new(kagra_shared::hop_beat::HopSpec::default());
        println!(
            "{}",
            serde_json::to_string_pretty(&g.world_value()).unwrap()
        );
        return;
    }
    let write = args.iter().any(|a| a == "--write");
    let spec = match args.iter().find(|a| !a.starts_with("--")) {
        Some(path) => {
            let text = std::fs::read_to_string(path).expect("read spec");
            let v: serde_json::Value = serde_json::from_str(&text).expect("json");
            let game = v.get("game").unwrap_or(&v);
            kagra_shared::hop_beat::HopSpec::from_value(game).expect("spec")
        }
        None => kagra_shared::hop_beat::HopSpec::default(),
    };
    let report = kagra_shared::hop_beat_measure::report(&spec);
    let text = serde_json::to_string_pretty(&report).unwrap() + "\n";
    if write {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/measure/hop_beat_baseline.json"
        );
        std::fs::write(path, &text).expect("write baseline");
        eprintln!("wrote {path}");
    } else {
        print!("{text}");
    }
}
