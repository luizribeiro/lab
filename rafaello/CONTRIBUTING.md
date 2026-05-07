# Contributing to rafaello

> **Status:** rafaello is in pre-implementation scaffolding. Most of
> this document is placeholder; concrete contribution guidance will
> land alongside the first non-stub code.

## Development environment

The repo uses a [devenv](https://devenv.sh/)-managed Nix shell. With
[direnv](https://direnv.net/) installed, `cd` into the repo and the
shell loads automatically. Otherwise:

```
nix develop .#rafaello
```

This provides Rust and the standard monorepo toolchain.

## Building

From the `rafaello/` directory:

```
cargo build --release
```

The binary lands at `rafaello/target/release/rfl`.

Or via Nix:

```
nix build .#rafaello
```

## Running tests

```
cargo test
```

## Design plans

In-flight design work lives under [`plans/`](./plans/), one directory
per stream. Each stream has a `README.md` describing the question and
a `notes.md` accumulating findings and proposed decisions. RFCs land
as named files in the same directory.
