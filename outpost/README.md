# outpost

Cross-consumer networking policy library for the lab monorepo.

`outpost` owns the host-allowlist DSL and policy model. Enforcement
backends consume it from their own crates:

- **capsa-vmnet** (in [capsa/](../capsa/)) — VM gateway that enforces
  policy via guest DNS interception + packet filtering on a smoltcp
  NAT'd virtual network.
- **outpost-proxy** — loopback HTTP(S) CONNECT proxy that enforces
  policy against CONNECT target authorities. Used by lockin's
  `proxy` network mode.

One policy vocabulary, two enforcement strategies. The same
`allow_hosts = ["api.example.com", "*.cdn.example.com"]` allowlist
means the same thing declaratively in both.
