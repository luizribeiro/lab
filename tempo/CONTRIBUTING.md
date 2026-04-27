# Contributing to tempo

## Development environment

The repo uses a [devenv](https://devenv.sh/)-managed Nix shell. With
[direnv](https://direnv.net/) installed, `cd` into the repo and the
shell is loaded automatically. Otherwise:

```
nix develop
```

This provides Rust, `cargo-dist`, and the other tools used by CI.

## Building

From the `tempo/` directory:

```
cargo build --release
```

The binary lands at `tempo/target/release/tempo`.

Or via Nix:

```
nix build .#tempo
```

## Running tests

```
cargo test
```

## Releasing

Releases are published by the unified `release.yml` workflow when a
tempo tag is pushed:

```
git tag tempo-vX.Y.Z
git push origin tempo-vX.Y.Z
```

Bump `version` in `crates/tempo/Cargo.toml` to match before tagging.
The workflow builds prebuilt binaries for macOS / Linux (x86_64 and
arm64) and pushes a Homebrew formula to `luizribeiro/homebrew-tap`.
