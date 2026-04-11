{ lib, pkgs, ... }:

let
  capsaPaths = import ./paths.nix { inherit lib pkgs; };
in
{
  packages = [ capsaPaths.libkrunPkg ]
    ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.sydbox ]
    ++ lib.optionals pkgs.stdenv.isDarwin [
    pkgs.libepoxy
    pkgs.virglrenderer
  ];

  env = {
    LIBKRUN_LIB_DIR = capsaPaths.libkrunLibDir;
    CAPSA_LIBRARY_DIRS = capsaPaths.libraryDirs;
  } // lib.optionalAttrs (capsaPaths.sydPath != null) {
    CAPSA_SYD_PATH = capsaPaths.sydPath;
  };
}
