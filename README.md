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

- Linux → `${libkrun}/lib` (and includes `syd` on `PATH`)
- macOS → `${libkrun-efi}/lib`

## Nix packages

The flake exposes:

- `.#capsa` (CLI package; sidecar stays private under `libexec`)
- `.#vm-assets` (kernel + initramfs for the default VM)
- `.#vm` (run script for the default VM built via `mkVM`)

Build default VM assets:

```bash
nix build .#vm-assets
```

Run the default VM directly:

```bash
nix run .#vm
```

## Defining custom VMs with Nix modules

The flake exposes:

- `lib.mkVM` → runnable package (`nix run` target)
- `lib.mkVMAssets` → VM assets/spec (`kernelImage`, `initramfsImage`, `vmAssets`)
- `lib.mkVMCheck` → expect-driven VM check derivation (`nix flake check`)

Define custom VMs in a NixOS-like style:

```nix
# in another flake
let
  vmArgs = {
    name = "demo";
    modules = [
      ({ ... }: {
        networking.hostName = "demo";
      })
    ];
    vm = {
      vcpus = 2;
      memoryMiB = 1024;
      kernelCmdline = "console=hvc0 rdinit=/init";
    };
  };

  capsaVm = capsa.lib.${system}.mkVM vmArgs;
  capsaVmAssets = capsa.lib.${system}.mkVMAssets vmArgs;
in {
  packages.${system}.demo-vm = capsaVm;
  packages.${system}.demo-vm-assets = capsaVmAssets.vmAssets;
}
```

`mkVMAssets` is useful when you want to run with Cargo directly:

```bash
cargo run -- \
  --kernel "${capsaVmAssets.kernelImage}" \
  --initramfs "${capsaVmAssets.initramfsImage}" \
  --kernel-cmdline "console=hvc0 rdinit=/init"
```

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

The helper binary is located at `crates/vmm/src/main.rs`.

- `cargo build --workspace` (or `cargo build --bins`) builds both `capsa` and `capsa-vmm`.
- Sidecar resolution order is:
  1. `CAPSA_VMM_PATH`
  2. sibling `capsa-vmm` next to the current executable
  3. `capsa-vmm` on `PATH`

Current backend status:

- **macOS**: implemented with `sandbox-exec` + generated Seatbelt profile
- **Linux**: `syd` integration (fail-closed). `syd` must be available on `PATH`; set `CAPSA_SANDBOX=off` to disable sandboxing explicitly.

VM smoke checks:

```bash
# Rust integration test
cargo test -p capsa-cli --test vm_smoke -- --nocapture

# Nix check (same boot/TTY/exit lifecycle)
nix flake check --print-build-logs
```

The smoke tests boot a VM, check interactive TTY I/O, send `exit`, and verify the process terminates. On Linux this runs with `syd` (required by default).
