{ pkgs, ... }:

{
  imports = [ ../git-hooks.nix ];

  languages.rust = {
    enable = true;
    toolchainFile = ../../../rust-toolchain.toml;
  };

  packages = [
    pkgs.pkg-config
    pkgs.cargo-dist
  ];
}
