{ lib, pkgs, ... }:

{
  # rafaello inherits the base Rust toolchain from shared/nix/devenv/base.nix.
  # m2 onward: rafaello-core's supervisor spawns subprocess plugins via lockin,
  # which needs the syd enforcer on Linux. Mirror lockin/nix/devenv.nix.
  env = lib.optionalAttrs pkgs.stdenv.isLinux {
    LOCKIN_SYD_PATH = "${pkgs.sydbox}/bin/syd";
    "CARGO_BIN_EXE_syd-pty" = "${pkgs.sydbox}/bin/syd-pty";
  };
}
