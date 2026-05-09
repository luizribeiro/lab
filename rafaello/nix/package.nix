{ pkgs
, src
, version ? "0.0.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "rafaello";
  inherit version src;

  cargoRoot = "rafaello";
  buildAndTestSubdir = "rafaello";

  cargoLock = {
    lockFile = src + "/rafaello/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "rafaello" ];

  doCheck = false;
}
