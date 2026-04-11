{ lib, pkgs }:
let
  libkrunPkg =
    if pkgs.stdenv.isDarwin then pkgs."libkrun-efi"
    else pkgs.libkrun;
in
rec {
  libkrunLibDir = "${lib.getLib libkrunPkg}/lib";

  libraryDirs = lib.makeLibraryPath (
    if pkgs.stdenv.isDarwin then
      [ pkgs.libiconv libkrunPkg ]
    else
      [ pkgs.glibc pkgs.stdenv.cc.cc pkgs.stdenv.cc.cc.libgcc libkrunPkg ]
  );

  sydPath =
    if pkgs.stdenv.isLinux then "${pkgs.sydbox}/bin/syd"
    else null;
}
