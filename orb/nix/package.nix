{ pkgs
, src
, version ? "0.1.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "orb";
  inherit version src;

  cargoRoot = "orb";
  buildAndTestSubdir = "orb";

  cargoLock = {
    lockFile = src + "/orb/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "orb" ];

  doCheck = false;
}
