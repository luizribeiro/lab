{ pkgs }:

let
  orbPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    orb = orbPackage;
  };

  checks = { };

  lib = { };
}
