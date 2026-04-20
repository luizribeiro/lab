{ lib, pkgs }:

let
  lockinPackage = import ./package.nix {
    inherit pkgs;
    src = ../..;
  };
  wrapWithLockin = import ./wrap.nix {
    inherit lib pkgs;
    lockin = lockinPackage;
  };
in
{
  packages = {
    lockin = lockinPackage;
  };

  lib = {
    inherit wrapWithLockin;
  };

  checks = {
    lockin-wrap-tests = import ./tests/wrap.nix {
      inherit lib pkgs wrapWithLockin;
    };
  };
}
