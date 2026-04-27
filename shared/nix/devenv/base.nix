{ pkgs, ... }:

{
  imports = [ ../git-hooks.nix ];

  languages.rust.enable = true;

  packages = [
    pkgs.pkg-config
    pkgs.cargo-dist
  ];
}
