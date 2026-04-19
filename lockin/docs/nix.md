# lockin Nix integration

`lockin/nix` exposes a `wrapWithLockin` helper that produces a
derivation whose `bin/*` entries run the original package under
lockin with a generated TOML policy.

From the flake:

```nix
# flake.nix of a downstream project
{
  inputs.lockin.url = "github:luizribeiro/lab";

  outputs = { self, nixpkgs, lockin, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system}.sandboxed-curl =
        lockin.lib.${system}.wrapWithLockin {
          package = pkgs.curl;
          policy = {
            sandbox.allow_network = true;
            filesystem.read_only_dirs = [ "/etc" "/nix/store" ];
            env.pass = [
              "PATH" "HOME" "USER" "TERM"
              "SSL_CERT_FILE" "NIX_SSL_CERT_FILE"
            ];
          };
        };
    };
}
```

`policy` is the same schema as the [CLI TOML config](cli.md#config-reference),
written as a Nix attrset. `wrapWithLockin` fills in two things
automatically for each binary in `${package}/bin`:

- `command = [ "/nix/store/.../bin/<name>" ]` — so the wrapper
  points at the real binary.
- `filesystem.library_paths` — derived by running `ldd` (Linux) or
  `otool -L` (Darwin) on the target binaries and collecting the
  `/nix/store` directories. Your own `library_paths` entries are
  preserved and merged.

On Linux the wrapper also sets `LOCKIN_SYD_PATH` to the `syd` from
nixpkgs, so the sandbox backend is found without any ambient
configuration.

## Arguments

| Arg | Type | Description |
|---|---|---|
| `package` | derivation | The package whose `bin/*` will be wrapped. |
| `policy` | attrset | Policy in the same shape as the TOML config. Optional; defaults to deny-all. |
| `name` | string | Derivation name. Defaults to `"<pname>-lockin"`. |
| `libraryDirs` | string \| null | Override auto-derivation with a colon-separated list. `null` (default) auto-derives. |
| `extraLibraryDirs` | list of paths | Appended to the auto-derived list. Useful when a binary loads plugins via `dlopen` that `ldd` won't see. |
| `sydPath` | path \| null | Override the `syd` binary used on Linux. `null` uses `pkgs.sydbox`. |

## Example: deny-by-default

```nix
lockin.lib.${system}.wrapWithLockin {
  package = pkgs.hello;
  # No policy: network denied, no filesystem access beyond what
  # the binary's own lib dirs need. A deny-all probe.
}
```

## Example: a service with state

```nix
lockin.lib.${system}.wrapWithLockin {
  package = pkgs.redis;
  policy = {
    sandbox.allow_network = true;
    filesystem.read_only_dirs = [ "/etc" "/nix/store" ];
    filesystem.read_write_dirs = [ "/var/lib/redis" ];
    limits.max_open_files = 4096;
    env.pass = [ "PATH" "HOME" ];
  };
}
```
