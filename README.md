# capsa

`capsa` starts a microVM using `libkrun`.

## Native dependencies

This project links to a platform-specific native library:

- **Linux**: `libkrun` (`-lkrun`)
- **macOS**: `libkrun-efi` (`-lkrun-efi`)

At build time, `build.rs` resolves the native library in this order:

1. `LIBKRUN_LIB_DIR` (if set)
2. `pkg-config`

If detection fails, the build prints a clear error with the expected library name.

## Nix development

Use the provided flake dev shell:

```bash
nix develop
```

The shell includes Rust tooling and exports `LIBKRUN_LIB_DIR` automatically:

- Linux → `${libkrun}/lib`
- macOS → `${libkrun-efi}/lib`

## Building VM assets (kernel + initramfs)

The flake exposes a minimal guest kernel/initramfs:

- `.#vm-kernel`
- `.#vm-initramfs`
- `.#vm-assets` (directory containing both)

Build both assets:

```bash
nix build .#vm-assets
```

This creates `./result/` with:

- `./result/vmlinuz`
- `./result/initramfs.cpio.lz4`

## Running

```bash
cargo run -- --help
```

You must provide a boot source:

- `--kernel <path>` (optionally with `--initramfs` and `--kernel-cmdline`), or
- `--root <dir>`

Example using the Nix-built assets:

```bash
cargo run -- \
  --kernel ./result/vmlinuz \
  --initramfs ./result/initramfs.cpio.lz4 \
  --vcpus 1 \
  --memory-mib 512
```

## Sandboxing

Sandboxing is always enabled. The CLI and library launch a dedicated sandboxed `capsa-vmm` subprocess automatically.

The helper binary is located at `src/bin/capsa-vmm.rs`.

- `cargo build --bins` builds both `capsa` and `capsa-vmm`.
- Sidecar resolution order is:
  1. `CAPSA_VMM_PATH`
  2. sibling `capsa-vmm` next to the current executable
  3. `capsa-vmm` on `PATH`

Current backend status:

- **macOS**: implemented with `sandbox-exec` + generated Seatbelt profile
- **Linux**: TODO placeholder for `syd` integration
