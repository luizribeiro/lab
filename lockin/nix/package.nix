{ pkgs
, src
, version ? "0.1.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "lockin";
  inherit version src;

  cargoRoot = "lockin";
  buildAndTestSubdir = "lockin";

  cargoLock = {
    lockFile = src + "/lockin/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "lockin-cli" ];

  doCheck = false;
}
