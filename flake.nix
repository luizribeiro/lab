{
  description = "lab monorepo";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devenv.url = "github:cachix/devenv";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = { nixpkgs, flake-utils, devenv, ... } @ inputs:
    flake-utils.lib.eachSystem [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ]
      (hostSystem:
        let
          inherit (nixpkgs) lib;
          pkgs = import nixpkgs {
            system = hostSystem;
            overlays = [
              (_final: prev:
                lib.optionalAttrs prev.stdenv.isLinux {
                  libkrun = prev.libkrun.override {
                    withNet = true;
                  };
                })
            ];
          };

          capsa = import ./capsa/nix {
            inherit lib pkgs nixpkgs hostSystem;
          };

          lockin = import ./lockin/nix {
            inherit lib pkgs;
          };

          outpost = import ./outpost/nix {
            inherit pkgs;
          };
        in
        {
          lib = capsa.lib // lockin.lib // outpost.lib;

          packages = capsa.packages // lockin.packages // outpost.packages // {
            default = capsa.packages.capsa;
          };

          checks = capsa.checks // lockin.checks // outpost.checks;

          devShells = {
            default = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/devenv/base.nix
                ./lockin/nix/devenv.nix
                ./capsa/nix/devenv.nix
              ];
            };

            lockin = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/devenv/base.nix
                ./lockin/nix/devenv.nix
              ];
            };

            fittings = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/devenv/base.nix
              ];
            };

            capsa = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/devenv/base.nix
                ./capsa/nix/devenv.nix
              ];
            };
          };

          formatter = pkgs.nixpkgs-fmt;
        });
}
