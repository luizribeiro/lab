{ pkgs }:

let
  scopePackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    scope = scopePackage;
  };

  checks = { };

  lib = { };
}
