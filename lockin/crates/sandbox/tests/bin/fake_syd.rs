//! Test fixture for inspecting how `lockin` invokes the `syd` child:
//! records argv and environ to the path named by
//! `RFL_FAKE_SYD_RECORD_PATH`, then exits 0 without doing any actual
//! sandboxing. Built only with `--features test-fixture`.

use std::env;
use std::fs;
use std::io::Write;

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() {
    let record_path = env::var_os("RFL_FAKE_SYD_RECORD_PATH")
        .expect("RFL_FAKE_SYD_RECORD_PATH must be set for fake-syd");

    let argv: Vec<String> = env::args().collect();
    let environ: Vec<String> = env::vars().map(|(k, v)| format!("{k}={v}")).collect();

    let argv_json = argv
        .iter()
        .map(|s| json_escape(s))
        .collect::<Vec<_>>()
        .join(",");
    let environ_json = environ
        .iter()
        .map(|s| json_escape(s))
        .collect::<Vec<_>>()
        .join(",");

    let blob = format!("{{\"argv\":[{argv_json}],\"environ\":[{environ_json}]}}");

    let mut f = fs::File::create(&record_path)
        .unwrap_or_else(|e| panic!("fake-syd: open {record_path:?}: {e}"));
    f.write_all(blob.as_bytes())
        .expect("fake-syd: write record");

    std::process::exit(0);
}
