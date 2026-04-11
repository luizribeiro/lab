{ lib, pkgs, ... }:

let
  seedPkg =
    if pkgs.stdenv.isDarwin then pkgs.libiconv
    else pkgs.stdenv.cc.cc;

  libraryDirs = import ../../shared/nix/runtime-lib-dirs.nix {
    inherit lib pkgs seedPkg;
  };
in
{
  env = {
    LOCKIN_LIBRARY_DIRS = libraryDirs;
  } // lib.optionalAttrs pkgs.stdenv.isLinux {
    LOCKIN_SYD_PATH = "${pkgs.sydbox}/bin/syd";
  };
}
