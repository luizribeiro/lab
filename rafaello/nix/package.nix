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

  cargoBuildFlags = [
    "-p"
    "rafaello"
    "-p"
    "rafaello-tui"
    "-p"
    "rafaello-openai"
    "-p"
    "rafaello-openai-stub"
    "-p"
    "rafaello-readfile"
    "-p"
    "rafaello-mailcat"
    "-p"
    "rafaello-mockprovider"
    "-p"
    "rafaello-fetch"
  ];

  doCheck = false;
}
