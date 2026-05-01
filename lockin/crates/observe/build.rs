fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/darwin_oslog.c")
            .compile("lockin_observe_darwin_oslog");
        println!("cargo:rerun-if-changed=src/darwin_oslog.c");
    }
}
