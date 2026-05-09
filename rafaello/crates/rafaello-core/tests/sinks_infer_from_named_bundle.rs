//! c18 — pi review-3 finding 3 / `decisions.md` row 17:
//!
//! Sink inference operates on the **per-tool effective grant**
//! (`default ∪ <tool-name>`). A tool whose `default` bundle has
//! no network/write authority but whose tool-named bundle adds
//! `write_dirs` must still infer `workspace_write` — otherwise
//! authority arriving via the named bundle would be invisible to
//! the sink classifier.

use rafaello_core::lock::{Grant, GrantBundle, GrantFilesystem};
use rafaello_core::sinks::{effective_grant, infer_defaults};

#[test]
fn named_bundle_write_dirs_lift_into_effective_for_inference() {
    let mut grant = Grant::default();

    grant.bundles.insert(
        "default".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                read_paths: vec!["${project}/**".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );

    grant.bundles.insert(
        "format".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                write_dirs: vec!["${project}/src".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );

    let effective = effective_grant(&grant, "format");

    let fs = effective.filesystem.as_ref().expect("union has filesystem");
    assert_eq!(fs.read_paths, vec!["${project}/**".to_owned()]);
    assert_eq!(fs.write_dirs, vec!["${project}/src".to_owned()]);

    assert_eq!(
        infer_defaults(&effective, &None),
        vec!["workspace_write".to_owned()],
        "default ∪ format yields workspace_write — row 17 / pi-3 finding 3"
    );
}

#[test]
fn unrelated_named_bundle_is_not_unioned_into_other_tool() {
    let mut grant = Grant::default();

    grant
        .bundles
        .insert("default".to_owned(), GrantBundle::default());
    grant.bundles.insert(
        "format".to_owned(),
        GrantBundle {
            filesystem: Some(GrantFilesystem {
                write_dirs: vec!["${project}/src".to_owned()],
                ..GrantFilesystem::default()
            }),
            ..GrantBundle::default()
        },
    );

    let effective_for_grep = effective_grant(&grant, "grep");
    assert!(
        infer_defaults(&effective_for_grep, &None).is_empty(),
        "format's write_dirs do not bleed into grep's effective bundle"
    );
}
