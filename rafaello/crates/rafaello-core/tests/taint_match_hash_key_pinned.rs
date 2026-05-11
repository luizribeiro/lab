use rafaello_core::reemit::taint_match::RFL_TAINT_MATCH_HASH_KEY;

#[test]
#[allow(clippy::unusual_byte_groupings)]
fn hash_key_pinned() {
    assert_eq!(
        RFL_TAINT_MATCH_HASH_KEY,
        (0xc0ffee_d00d_f00d_b002_u128 as u64, 0xa11ce_b0b_face_b00c,)
    );
}
