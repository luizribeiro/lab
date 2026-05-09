Adversarial review of `rafaello/plans/milestones/m1-manifest/retrospective.md`.

## Findings

### 1. **Blocking: Stream A drift is incorrectly downgraded to “recorded only”**

The retrospective says Stream A helper/external-attach drift should not be patched because “Stream A is not Stream F” (§2.3/§2.4/Follow-up commits). That contradicts the ratified milestone docs:

- `milestones/README.md` §"Stream RFC drift" says `streams/a-security/rfc-security-model.md` still describes helper plugins, external attach, and `requires_confirmation`, and says it is **“Patched in the m1 retrospective”**.
- `m1-manifest/scope.md` §Acceptance summary says the m1 retrospective specifically owns the Stream F drift items **plus** the security RFC rename/helper/external-attach drift.
- `driver-notes.md` “After c37 lands” says to apply the Stream F body patches **and** the security RFC rename/helper/external-attach drift.

So the retrospective’s “no Stream A patch” decision is not just a policy interpretation; it is a change to ratified scope. Either patch `streams/a-security/rfc-security-model.md` in the m1 follow-up sweep, or explicitly get owner ratification to change the milestone contract. I would not sign off while this is recorded-only.

Related factual issue: §2.2 says a grep for `requires_confirmation` has no hits except `overview.md`. That is false: `streams/a-security/rfc-security-model.md` and `milestones/README.md` both still contain it.

### 2. **Blocking: fittings workspace acceptance is not green in the captured evidence**

The retrospective marks the §W acceptance as green “modulo the pre-existing m0 flake” and says a pi-5 finding allowed that. I could not find such a carve-out in the m1 pi reviews, and the ratified scope/commits acceptance text requires:

```text
cargo test --manifest-path fittings/Cargo.toml --workspace green
```

`manual-validation.md` records the opposite: 229 passed, 1 failed. The failure may be pre-existing, but the acceptance gate as written is still not satisfied by that capture.

Fix: rerun until a clean fittings workspace pass is captured, or get an explicit owner waiver / scope amendment saying the known m0 flake is excluded. Do not mark the gate green based on a failing run.

### 3. **Medium: matrix counts are not reproducible**

The retrospective says “47 positive” and “71 negative” scope-named tests. Counting table rows in `scope.md` gives different numbers (positive table: 38 rows; negative table: 86 rows, with some positive-behaviour rows living under the negative heading). The landed-file coverage appears broadly correct aside from the documented `digest_match_compiles.rs` alias, but the exact counts in the retrospective are false precision and undermine the “mechanically verified” claim.

Fix: replace the hard-coded 47/71 counts with a reproducible inventory, or correct the counts and explain any reclassification of rows from the negative table.

### 4. **Medium: final status is internally inconsistent about acceptance blockers**

The retrospective correctly notes `cargo doc` is technically unmet due to the `Error` intra-doc-link warning, and lists a follow-up fix. But elsewhere it says the c37 acceptance gate is met and that m1 is done pending follow-up commits. Since `cargo doc --no-deps` warning-free is an explicit acceptance bullet, this should be phrased as “not yet accepted until follow-up #3 lands and doc is rerun clean,” not as a green gate.

Same issue applies to the Stream A/F doc-drift follow-ups: if they are acceptance-owned by the retrospective, they are not optional cleanup after sign-off.

### 5. **Low: traceability prose overclaims commit-plan fidelity**

The retrospective says all trace tables were mechanically verified. There are still stale/misleading trace details in the plan material, e.g. `commits.md` maps `manifest_grant_match_present.rs` to c10 in one trace table while the file actually landed in `1d84452`/c11. This does not look like a coverage gap, but the retrospective should not imply the commit mapping is fully clean unless these stale rows are corrected or explicitly ignored as non-normative.

## Verdict

Do not sign off on the retrospective as written. The main blockers are (1) the missed/incorrectly-deferred Stream A RFC drift that ratified docs assign to m1, and (2) acceptance gates being marked green despite failing captured evidence (`fittings` workspace, plus `cargo doc` until its follow-up lands).
