# Pi review 3 — sign-off review of revised rafaello v1 milestones

Review target: the milestone roadmap and architecture patches after
`pi-review-2.md`, especially:

- `91ec3b6 docs(rafaello-decisions): defer public rfl serve to v2 (row 34)`
- `70eef1c docs(rafaello-overview): add v1 deferral callouts to §§9, 10, 11, 12, 13`
- `24a8b33 docs(rafaello-overview): finish §1, §3, §5, §15 drift cleanup`
- `e57d4b2 docs(rafaello-milestones): apply pi-review-2 must-fixes 2-4`

Verdict: **not quite sign-off yet — one small but real consistency
blocker remains.**

Claude fixed the roadmap substance well. The milestone README is now
basically sign-offable: m3 drops daemon/public serve, m4 includes the
mandatory taint envelope, m5 layers taint matching/gating on top, stream
RFC drift is explicitly tracked, and TUI principal / fixture-entry /
renderer panic isolation are clarified.

The remaining blocker is a source-of-truth contradiction in `overview.md`
around public `rfl serve`.

## Per-finding verdicts from round 2

### R2 must-fix 1 — patch `overview.md` beyond §16

**Verdict: mostly fixed, but one §16 contradiction remains.**

The new overview patches add useful v1-status callouts in the earlier
sections:

- §1 now explains that multiple-frontends/daemon attach are v2 while TUI
  remains v1.
- §3 now marks helper plugin, external frontend attach, and subprocess
  renderer boxes as v2/full-picture only.
- §5 now calls out manifest simplifications and helper deferral.
- §§9–12 now have v1-status callouts for helper plugins, frontends,
  renderers/streaming, and daemon mode.
- §13 now warns that the older decision mirror is superseded by rows
  26–34.
- §15 now has a v1-status note on deferred manifest/helper/renderer/
  streaming items.

This resolves most of the round-2 concern. The remaining issue is that
`overview.md` §16 and some nearby old prose still list public `rfl serve`
as v1. Details are in the blocking finding below.

### R2 must-fix 2 — clarify or patch stream RFC drift

**Verdict: fixed for roadmap sign-off.**

The milestone README now has an explicit “Stream RFC drift” section. It
lists known stale Stream A, Stream E, and Stream F sections and states
that `overview.md` wins, with the drift patched in the relevant milestone
retrospectives.

This is acceptable at roadmap granularity. Stream F drift should still be
patched during m1 because m1 implements the schema, but the roadmap now
warns implementers clearly enough.

### R2 must-fix 3 — move mandatory taint envelope into m4

**Verdict: fixed.**

m4 now includes:

- canonical `taint` envelope on `core.session.tool_request` and
  `core.session.tool_result`;
- envelope presence and structural validation;
- core-produced taint, not plugin-supplied taint;
- negative tests for missing taint and plugin-supplied taint.

m5 now correctly adds matching/propagation rules and the broker-side sink
gate that consumes the envelope. This is the right staging.

### R2 must-fix 4 — define or defer public `rfl serve` for v1

**Verdict: fixed in decisions and milestones; still stale in `overview.md`
§16.**

Decision row 34 correctly defers public `rfl serve` to v2. The milestone
README now says m3 is `rfl chat` running core + TUI in one process tree,
with no daemon and no attach socket in v1.

But `overview.md` still has contradictory v1-scope text listing `serve`.
This is the only remaining sign-off blocker.

## Blocking finding: `overview.md` §16 still contradicts row 34

**Severity: blocking for sign-off, small patch required.**

`decisions.md` row 34 says:

> Public `rfl serve` deferred to v2. v1 has no daemon command; `rfl chat`
> runs the agent core and the TUI together in one process tree.

The revised milestone README matches that:

- m3: “`rfl chat` spawns core + the bundled `rafaello-tui` … no daemon,
  no attach socket in v1.”
- m3 demo: `rfl chat` opens the TUI.
- round-2 change log: public `rfl serve` is v2.

However, `overview.md` still contains stale public-serve wording:

1. **§3 process model old prose still says `rfl serve` exposes an attach
   socket in v1.**

   The v1-status callout above the process diagram is good, but the old
   prose below still says core has no out-of-process daemon separate from
   agent and that `rfl serve` is the same binary with the attach socket
   exposed. After row 34, that sentence must be v2-only or removed from
   v1 prose.

2. **§12 has a correct v1-status callout, but old prose remains below.**

   The callout says public `rfl serve` and the attach socket are deferred
   to v2. The following explanatory paragraph still describes the binary
   always exposing the attach socket and daemon mode waiting for external
   frontends. The paragraph is labeled “v2” in one parenthetical, but it
   is still easy to read as part of the v1 architecture. It should be
   rewritten so v1 is direct and v2 is explicitly future.

3. **§16 still lists `serve` in v1 CLI subcommands.**

   Current §16 v1 scope cut includes:

   ```text
   CLI subcommands `rfl init / install / grant / revoke
   / update / provider use / status / serve`
   ```

   This directly contradicts row 34 and the milestone README. Since §16
   is the canonical v1 scope cut, this must be patched before sign-off.

4. **§16 deferred table still implies `rfl serve` / UDS attach are v1.**

   The deferred table still says:

   - “Multi-session daemon — Single-session `rfl serve` is the v1 shape”
   - “Network-attached frontends (TCP) — UDS-only is the v1 attach surface”

   After rows 27 and 34, the correct v1 shape is no public daemon and no
   attach surface. The deferred entries should say public daemon / attach
   socket / external frontend attach are v2, and perhaps distinguish TCP
   network frontends as later than or part of that v2 work.

This is exactly the kind of source-of-truth contradiction round 2 asked
to eliminate. Because `overview.md` wins over stream RFCs and guides
implementation, it must not simultaneously say public `rfl serve` is both
v1 and v2.

## Non-blocking cleanups

1. **Milestone README status is stale.**

   It still says “Pi review 2 pending.” Update to “Pi review 3 pending”
   or, after the §16 patch and this review, “pi review 3 signed off;
   owner ratification pending.”

2. **`overview.md` §13 note has confusing item references.**

   The note says rows 26–34 reverse “Items 9 (helper plugins), 14
   (frontend principals beyond TUI), 15 (RenderTree subprocess
   renderers), and 16 (streaming patch ops).” The numbering does not
   match the list below in an obvious way: helper plugins are item 14,
   frontends item 15, render tree item 19, streaming topics item 20.

   This is not a blocker because the note points readers to §16 and
   `decisions.md`, but it should be cleaned up to avoid confusing future
   agents.

3. **Some sections keep old “eventual/v2” prose after v1 callouts.**

   This is acceptable now that callouts and stream-drift tracking exist,
   but direct “in v1” claims should be removed where practical. The main
   direct contradiction is `rfl serve`; other future-shape prose can stay
   if clearly labeled as eventual/v2.

4. **Decision rows still say “Owner-approved” while status is
   `proposed`.**

   This is non-blocking for the roadmap review, but if owner approval has
   happened, consider marking relevant rows `ratified`; otherwise avoid
   phrasing that implies ratification before the status changes.

## Sign-off condition

Patch `overview.md` so the public-serve story is consistent everywhere:

- v1 public CLI excludes `rfl serve`;
- v1 entrypoint is `rfl chat`, running core + TUI in one process tree;
- v1 has no public daemon command, no attach socket, and no external
  frontend attach;
- public daemon / attach socket / external frontend attach are v2.

Concretely, patch §3 stale process prose, §12 interactive-vs-daemon
prose, and §16 v1/deferred tables. Also update the milestone README
status line if desired.

After that patch, I would sign off on the milestones overview for owner
ratification.
