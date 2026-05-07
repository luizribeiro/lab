# rafaello v1 architecture (overview)

> **Status:** stub. Authored by claude+pi iteration before milestone
> scoping. The project owner ratifies on convergence.
>
> This is the single source of truth for v1. If anything in
> `streams/` conflicts with what is written here, this document wins
> and the relevant stream RFC gets a follow-up edit logged in the
> next milestone's retrospective.

## Sections (to be filled)

1. Goals and non-goals
2. Trust model and security posture
3. Process model — agent core, daemon, frontends, plugins
4. Bus and event model
5. Plugin lifecycle — manifest → lock → policy → spawn → lazy load
6. Tool dispatch and the LLM-untrusted-output gate
7. Provider model
8. Renderer model and frontend protocol
9. Sessions, persistence, branching
10. Daemon mode and frontends-over-RPC
11. v1 scope cut — what's in, what's deferred
12. Reference index — pointers into `streams/` for detail
