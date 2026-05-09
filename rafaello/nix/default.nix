{ pkgs }:

let
  rafaelloPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    rafaello = rafaelloPackage;
  };

  checks = { };

  lib = { };
}
