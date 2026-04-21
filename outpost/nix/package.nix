{ pkgs
, src
, version ? "0.1.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "outpost";
  inherit version src;

  cargoRoot = "outpost";
  buildAndTestSubdir = "outpost";

  cargoLock = {
    lockFile = src + "/outpost/Cargo.lock";
  };

  cargoBuildFlags = [ "-p" "outpost" ];

  doCheck = false;
}
