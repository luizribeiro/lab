{ pkgs, ... }:

{
  git-hooks.hooks = {
    rustfmt.enable = true;

    clippy = {
      enable = true;
      packageOverrides = {
        inherit (pkgs) cargo clippy;
      };
      settings = {
        denyWarnings = true;
        extraArgs = "--target-dir target/pre-commit";
      };
    };

    nixpkgs-fmt.enable = true;
    statix.enable = true;
    deadnix.enable = true;
  };
}
