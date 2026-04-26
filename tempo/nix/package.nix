{ pkgs
, src
, version ? "0.1.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "tempo";
  inherit version src;

  cargoRoot = "tempo";
  buildAndTestSubdir = "tempo";

  cargoLock = {
    lockFile = src + "/tempo/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "tempo" ];

  doCheck = false;
}
