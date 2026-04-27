{ pkgs
, src
, version ? "0.1.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "scope";
  inherit version src;

  cargoRoot = "scope";
  buildAndTestSubdir = "scope";

  cargoLock = {
    lockFile = src + "/scope/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "scope" ];

  doCheck = false;
}
