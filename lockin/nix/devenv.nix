{ lib, pkgs, ... }:

let
  seedPkg =
    if pkgs.stdenv.isDarwin then pkgs.libiconv
    else pkgs.stdenv.cc.cc;

  testExecDirs = import ../../shared/nix/runtime-lib-dirs.nix {
    inherit lib pkgs seedPkg;
  };
in
{
  env = {
    LOCKIN_TEST_EXEC_DIRS = testExecDirs;
  } // lib.optionalAttrs pkgs.stdenv.isLinux {
    LOCKIN_SYD_PATH = "${pkgs.sydbox}/bin/syd";
  };
}
