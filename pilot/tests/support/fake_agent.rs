use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let mut script_path: Option<String> = None;
    while let Some(a) = args.next() {
        if a == "--script" {
            script_path = args.next();
        }
    }
    let path = script_path.expect("--script <file> is required");
    let contents = fs::read_to_string(&path).expect("read script");
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stderr = io::stderr();
    let mut err = stderr.lock();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("emit ") {
            writeln!(out, "{rest}").unwrap();
            out.flush().unwrap();
        } else if let Some(rest) = line.strip_prefix("stderr ") {
            writeln!(err, "{rest}").unwrap();
            err.flush().unwrap();
        } else if let Some(rest) = line.strip_prefix("exit ") {
            let code: i32 = rest.trim().parse().unwrap_or(0);
            process::exit(code);
        }
    }
}
