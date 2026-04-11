{ lib, pkgs }:
let
  libkrunPkg =
    if pkgs.stdenv.isDarwin then pkgs."libkrun-efi"
    else pkgs.libkrun;
in
rec {
  inherit libkrunPkg;
  libkrunLibDir = "${lib.getLib libkrunPkg}/lib";

  libraryDirs = import ../../shared/nix/runtime-lib-dirs.nix {
    inherit lib pkgs;
    seedPkg = libkrunPkg;
  };

  sydPath =
    if pkgs.stdenv.isLinux then "${pkgs.sydbox}/bin/syd"
    else null;
}
