# Stream A — Security model

## The question

What is rafaello's complete security posture, and what concrete v1
primitives enable it without baking policy into core?

The starting position (drafted in the design conversation):

- The agent process itself is trusted code we wrote, but **the LLM
  is not trusted at any level**. Every action the LLM can take is
  manifest-gated, including built-in tools.
- Three artifacts: plugin **manifest** (request) → user-edited
  **lock** (grant) → **lockin policy** (runtime enforcement). The AI
  cannot mutate the lock; only `rfl install`/`rfl grant` can.
- Plugins identified by `<source>:<name>@<version>` plus content
  digest. Manifest changes prompt re-confirmation.
- Project-scoped: tools default to the project root; broader scope
  requires explicit grant.
- Bus access controlled by manifest-declared subscribe/publish
  topics; core events are host-published only.

## Open work

- Stress-test the trust model end to end: can the AI subvert any of
  it? Permission flow, lock file, sandbox escape paths.
- Bus ACL design: how does rafaello validate "this plugin is allowed
  to subscribe to this topic" without becoming a policy DSL?
- Lockin sufficiency: what classes of plugin need stricter isolation
  (capsa VMs)? What's the v1 vs v2 cut?
- CaMeL-as-plugin viability: write a draft prompt for a v2 agent
  that builds CaMeL on top of v1 primitives only. If the prompt
  cannot be written without referencing things not in v1, identify
  the gap.

## Deliverables

- `rfc-security-model.md` — complete v1 security model, with concrete
  capability/permission surfaces, attack scenarios, and mitigations.
- `rfc-camel-on-v1.md` — explicit v2 prompt demonstrating CaMeL is
  buildable from v1 primitives alone.

## Inputs

- Conversation history with the human (project owner).
- pi-mono source at `/tmp/pi-mono/` if present.
- CaMeL paper: arXiv:2503.18813.
- lockin TOML schema and documentation under `lockin/docs/`.
