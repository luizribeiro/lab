{ pkgs, ... }:

let
  workspaces = [ "capsa" "fittings" "lockin" ];
  wsFlags = builtins.concatStringsSep " " workspaces;

  clippy-monorepo = pkgs.writeShellScript "clippy-monorepo" ''
    export PATH="${pkgs.cargo}/bin:${pkgs.clippy}/bin:$PATH"
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
    export PATH="${pkgs.cargo}/bin:${pkgs.rustfmt}/bin:${pkgs.rustc}/bin:$PATH"
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
