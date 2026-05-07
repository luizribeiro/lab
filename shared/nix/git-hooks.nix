{ pkgs, config, ... }:

let
  workspaces = [ "capsa" "fittings" "lockin" ];
  wsFlags = builtins.concatStringsSep " " workspaces;

  # The clippy/rustfmt hooks must use the exact same toolchain as the
  # dev shell (pinned via rust-toolchain.toml). Referencing pkgs.cargo
  # / pkgs.clippy directly here would silently track nixpkgs and
  # re-introduce the drift this file is meant to prevent.
  rustToolchain = config.languages.rust.toolchain;

  clippy-monorepo = pkgs.writeShellScript "clippy-monorepo" ''
    export PATH="${rustToolchain.cargo}/bin:${rustToolchain.clippy}/bin:${rustToolchain.rustc}/bin:$PATH"
    for ws in ${wsFlags}; do
      cargo clippy \
        --manifest-path "$ws/Cargo.toml" \
        --all-targets \
        --target-dir "$ws/target/pre-commit" \
        -- -D warnings \
        || exit 1
    done
  '';

  rustfmt-monorepo = pkgs.writeShellScript "rustfmt-monorepo" ''
    export PATH="${rustToolchain.cargo}/bin:${rustToolchain.rustfmt}/bin:${rustToolchain.rustc}/bin:$PATH"
    for ws in ${wsFlags}; do
      cargo fmt --all --manifest-path "$ws/Cargo.toml" -- --check --color always \
        || exit 1
    done
  '';
in
{
  git-hooks.hooks = {
    rustfmt = {
      enable = true;
      entry = builtins.toString rustfmt-monorepo;
      pass_filenames = false;
    };

    clippy = {
      enable = true;
      entry = builtins.toString clippy-monorepo;
      pass_filenames = false;
    };

    nixpkgs-fmt.enable = true;
    statix.enable = true;
    deadnix.enable = true;
  };
}
