{ pkgs }:

let
  outpostPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    outpost = outpostPackage;
  };

  checks = { };

  lib = { };
}
