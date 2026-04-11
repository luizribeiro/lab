{ lib, pkgs }:
let
  libkrunPkg =
    if pkgs.stdenv.isDarwin then pkgs."libkrun-efi"
    else pkgs.libkrun;
in
{
  libraryDirs =
    if pkgs.stdenv.isDarwin then
      lib.concatStringsSep ":" [
        "${lib.getLib pkgs.libiconv}/lib"
        "${lib.getLib libkrunPkg}/lib"
      ]
    else
      lib.concatStringsSep ":" [
        "${lib.getLib pkgs.glibc}/lib"
        "${lib.getLib pkgs.stdenv.cc.cc}/lib"
        "${pkgs.stdenv.cc.cc.libgcc}/lib"
        "${lib.getLib libkrunPkg}/lib"
      ];

  libkrunLibDir = "${lib.getLib libkrunPkg}/lib";

  sydPath =
    if pkgs.stdenv.isLinux then "${pkgs.sydbox}/bin/syd"
    else null;
}
