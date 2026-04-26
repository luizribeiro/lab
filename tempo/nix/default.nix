{ pkgs }:

let
  tempoPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    tempo = tempoPackage;
  };

  checks = { };

  lib = { };
}
