use std::env;
use std::path::Path;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let lib_name = match target_os.as_str() {
        "macos" => "krun-efi",
        "linux" => "krun",
        other => panic!("unsupported target OS for capsa/libkrun: {other}"),
    };

    if let Ok(lib_dir) = env::var("LIBKRUN_LIB_DIR") {
        if !Path::new(&lib_dir).exists() {
            panic!("LIBKRUN_LIB_DIR does not exist: {lib_dir}");
        }

        println!("cargo:rustc-link-search=native={lib_dir}");
        println!("cargo:rustc-link-lib={lib_name}");
        if target_os == "macos" {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
        }
        return;
    }

    match pkg_config::Config::new().probe(lib_name) {
        Ok(_) => {}
        Err(err) => {
            panic!(
                "failed to find native library '{lib_name}' via pkg-config ({err}). \
Install the native dependency (Linux: libkrun, macOS: libkrun-efi) \
or set LIBKRUN_LIB_DIR to the directory containing lib{lib_name}."
            );
        }
    }
}
