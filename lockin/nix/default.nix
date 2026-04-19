{ lib, pkgs }:

let
  lockinPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
in
{
  packages = {
    lockin = lockinPackage;
  };

  lib = {
    wrapWithLockin = import ./wrap.nix {
      inherit lib pkgs;
      lockin = lockinPackage;
    };
  };
}
