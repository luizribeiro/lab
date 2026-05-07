# notes — stream-a-security

Working notes; findings appended as the RFC develops.

## F1. Threat model framing

The interesting adversary is not "a malicious plugin author who
publishes a clearly-evil plugin." It is the **vibe-coded plugin
that wasn't audited**, plus the **prompt injection landing in a
tool result** that turns any plugin (well-meaning or not) into an
exfiltration vector.

The trust boundary is therefore drawn around three actors:

1. **Agent core (`rfl`).** Trusted. Wrote it ourselves.
2. **The LLM and any data it produces or has touched.** Untrusted.
   This is the key axiom: even when "the LLM" is the user's own
   provider, the model's output is treated as adversarial input
   for the purposes of capability decisions.
3. **Plugin code.** Semi-trusted at install time (the user clicked
   "yes"), but never trusted at runtime — every plugin runs under
   a lockin policy compiled from its locked grant, not its current
   manifest.

The "manifest is a request, lock is the grant" pattern means the
plugin author cannot escalate by editing their own manifest after
the fact: the lock is the source of truth and only `rfl install`
/ `rfl grant` may write it.

## F2. The lethal trifecta as the central design constraint

Simon Willison's framing — *untrusted data + tool access + outbound
communication = exfiltration* — is the single hardest problem and
the one rafaello cannot avoid because it is, definitionally, a
coding agent. We have all three.

The structural answer is to break **at least one leg of the
trifecta per plugin**, not per agent. Concretely:

- A plugin that reads untrusted data (web fetch, GitHub issue
  body, file from `/tmp`) must not also have outbound network in
  the same lockin policy.
- A plugin that has outbound network must not also have read
  access to the project workspace.
- A plugin with both must not be invokable directly by the LLM
  without an explicit per-call confirmation.

This generalises into a **taint propagation rule** on the bus,
which CaMeL-as-plugin then makes formal (Stream A v2). For v1 we
just need the primitives — taint tags on bus events and per-plugin
egress allowlists — to make the rule expressible.

## F3. Bus ACL: manifest-as-DSL, not new DSL

The instinct to invent a policy DSL for "plugin X can subscribe
to topic Y" is the wrong one. The manifest already declares
`subscribes` and `publishes`. The broker just enforces what's
already there — no separate ACL file, no policy language. This
keeps lock-the-grant as the single source of truth.

The interesting subtlety is **core-published topics** (e.g.
`session.user_message`, `session.tool_result`). Plugins may
subscribe to those if granted, but cannot publish on them. The
broker tags every event at publish time with the publisher's
plugin id; core events are tagged `core` and any plugin trying to
publish a `core`-namespaced topic is rejected at the broker, not
at the manifest layer.

## F4. Lockin sufficiency vs. capsa VMs

Lockin gives us: filesystem allowlist, network proxy with hostname
allowlist, env scrubbing, exec gating, rlimits. What it does NOT
give us:

- **CPU/memory isolation from the host kernel.** Side-channel
  attacks, kernel exploits, anything that needs hardware isolation.
- **Filesystem snapshot isolation.** A plugin with `write_dir =
  ["./output"]` can corrupt that directory; lockin doesn't snapshot.
- **Tamper-evident execution.** Lockin can't attest "this is the
  binary you locked"; the hash is checked at install, not at exec.

For v1 we accept these gaps because the alternative (capsa VMs
per plugin) is months of work. The v1/v2 cut is: **anything that
takes user secrets or network credentials runs under lockin only;
anything that runs untrusted-third-party model output as code
goes to capsa in v2** (e.g. a future `python-eval` plugin).

## F5. The CaMeL gap

CaMeL needs three primitives from v1 to be implementable as a
pure plugin:

1. A way to register as a **provider** so it sees prompts before
   they reach the real model, and tool results before they reach
   the user-visible LLM.
2. A way to **invoke tools through the bus** as if the plugin
   were the agent (so the quarantined LLM's outputs route through
   CaMeL's privileged-LLM-issued calls).
3. A way to **attach taint metadata** to tool results that
   downstream plugins (and CaMeL itself) read.

Stream F (manifest) covers (1) — `provides: provider`. Stream B
(fittings RPC + bus) covers (2). The only thing v1 must
specifically commit to for (3) is a **standard envelope field**
on tool-result bus events — `taint: [<source-id>...]` — so CaMeL
isn't reverse-engineering it from string heuristics.

If we don't add the taint envelope to the bus event schema, CaMeL
is buildable but every plugin author would have to opt in. That's
the v1 commitment that unlocks v2 cleanly.
