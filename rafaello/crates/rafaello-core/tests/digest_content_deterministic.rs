use std::fs;

use rafaello_core::digest::content_digest;

fn build_fixture_in_order(root: &std::path::Path, order: &[(&str, &[u8])]) {
    for (rel, bytes) in order {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, bytes).unwrap();
    }
}

#[test]
fn content_digest_is_independent_of_file_creation_order() {
    let order_a: &[(&str, &[u8])] = &[
        ("src/lib.rs", b"pub fn one() {}\n"),
        ("README.md", b"# pkg\n"),
        ("src/util/mod.rs", b"pub mod helpers;\n"),
        ("src/util/helpers.rs", b"pub fn h() {}\n"),
        ("manifest.toml", b"name = \"pkg\"\n"),
    ];
    let order_b: &[(&str, &[u8])] = &[
        ("manifest.toml", b"name = \"pkg\"\n"),
        ("src/util/helpers.rs", b"pub fn h() {}\n"),
        ("src/lib.rs", b"pub fn one() {}\n"),
        ("src/util/mod.rs", b"pub mod helpers;\n"),
        ("README.md", b"# pkg\n"),
    ];

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    build_fixture_in_order(dir_a.path(), order_a);
    build_fixture_in_order(dir_b.path(), order_b);

    let digest_a = content_digest(dir_a.path()).unwrap();
    let digest_b = content_digest(dir_b.path()).unwrap();

    assert_eq!(digest_a, digest_b);
    assert!(digest_a.starts_with("sha256:"));
    assert_eq!(digest_a.len(), "sha256:".len() + 64);
}
