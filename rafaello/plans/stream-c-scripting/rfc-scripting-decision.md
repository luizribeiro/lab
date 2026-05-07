# RFC — embedded scripting in rafaello v1

**Stream:** C (scripting)
**Status:** draft, single-author research
**Author:** stream-c agent

## TL;DR

**Recommendation: do NOT include an embedded scripting language in
rafaello v1.** Customisation is served by three planes:

1. **Declarative config** (TOML) for keymaps, prompt templates,
   hook→event wiring, statusline composition, theme.
2. **Subprocess plugins** speaking JSON-RPC over the bus, sandboxed
   by lockin from a manifest, for anything that needs code.
3. **Headless `rfl` driven over the bus** by external processes for
   evals, alternate frontends, and loop replacement.

The embedded plane (Luau via `mlua`) is deferred to v2 and only
revisited if real usage shows the declarative plane cannot hold the
"tiny customization" cases without forcing users to write a plugin
crate.

The single strongest argument: every motivation for the embedded
plane (keymaps, hooks, templates, statusline) collapses into
*declarative configuration that core already has to parse anyway*,
and the cases that are not declarative — custom tools, custom
renderers, the agent loop — are exactly the cases where lockin
isolation pays for itself. Adding Luau is therefore solving a
problem that doesn't exist (declarative cases) and *under*-solving
a problem that does (the trusted-config-vs-untrusted-plugin split is
clearer with one runtime model than two).

The rest of this document walks four UX scenarios in both worlds,
then tallies cost/benefit, then specifies the v1 declarative surface
that the recommendation depends on.

## Framing: three planes, not two

The pushback the project owner raised — "if everything talks the
bus, any language can extend rafaello" — is correct, but it
collapses two questions into one:

1. *What language do plugins (capability-bearing extensions) write
   in?* Answer: any language that can speak JSON-RPC. Already
   settled by the bus design.
2. *What does the user write to bend rafaello to their taste
   without authoring a plugin?* This is the open question.

Neovim answers (2) with Lua. pi answers (2) with TypeScript for
extensions but with **plain files** (`keybindings.json`,
`*.md` prompts, `*.md` skills, `settings.json` shell prefix) for
the truly small cases. Pi is closer to the right answer for
rafaello, because rafaello's value proposition is "secure by
default, very few footguns" and a Turing-complete in-process plane
is by definition a footgun surface — even a sandboxed one.

The third plane is **headless rfl driven from outside**: an eval
harness, an alternate frontend, a CaMeL supervisor. This is also
the bus, but it inverts the relationship: external code drives
rfl, not vice versa. We need to be sure this works for v1
regardless of the embedded-language question.

