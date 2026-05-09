fn main() {
    let mode = std::env::var("RFL_FIXTURE_MODE").unwrap_or_default();
    match mode.as_str() {
        "scaffold_only" => {}
        _ => {
            eprintln!("rfl-bus-fixture: unknown mode '{}'", mode);
            std::process::exit(64);
        }
    }
}
