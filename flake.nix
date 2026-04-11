{
  description = "lab: monorepo for capsa, fittings, and friends";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devenv.url = "github:cachix/devenv";
    git-hooks.url = "github:cachix/git-hooks.nix";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = { self, nixpkgs, flake-utils, devenv, git-hooks, ... } @ inputs:
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

          # --- capsa build infrastructure ---
          capsaPaths = import ./capsa/nix/paths.nix { inherit lib pkgs; };

          capsaPackage = import ./capsa/nix/package.nix {
            inherit lib pkgs capsaPaths;
            src = ./capsa;
          };

          vmLib = import ./capsa/nix/vm {
            inherit lib pkgs nixpkgs hostSystem capsaPackage capsaPaths;
          };

          defaultVmAssets = vmLib.mkVMAssets { name = "capsa"; };
          defaultVm = vmLib.mkVM { name = "capsa"; };

          capsaChecks = import ./capsa/nix/checks {
            inherit vmLib pkgs capsaPaths;
          };
        in
        {
          # --- capsa library outputs ---
          lib = {
            inherit (vmLib) mkVMAssets mkVM mkVMCheck;
          };

          # --- packages ---
          packages = {
            capsa = capsaPackage;
            vm-assets = defaultVmAssets.vmAssets;
            vm = defaultVm;
            default = self.packages.${hostSystem}.capsa;
          };

          # --- checks ---
          checks = capsaChecks;

          # --- dev shells ---
          devShells = {
            capsa = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/git-hooks.nix
                ({ lib, pkgs, ... }: {
                  languages.rust.enable = true;

                  packages = [ pkgs.pkg-config ]
                    ++ lib.optionals pkgs.stdenv.isLinux [
                    pkgs.libkrun
                    pkgs.sydbox
                  ]
                    ++ lib.optionals pkgs.stdenv.isDarwin [
                    pkgs."libkrun-efi"
                    pkgs.libepoxy
                    pkgs.virglrenderer
                  ];

                  env = {
                    LIBKRUN_LIB_DIR = capsaPaths.libkrunLibDir;
                    CAPSA_LIBRARY_DIRS = capsaPaths.libraryDirs;
                  } // lib.optionalAttrs (capsaPaths.sydPath != null) {
                    CAPSA_SYD_PATH = capsaPaths.sydPath;
                  };
                })
              ];
            };

            fittings = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/git-hooks.nix
                {
                  languages.rust.enable = true;
                }
              ];
            };

            default = self.devShells.${hostSystem}.capsa;
          };

          formatter = pkgs.nixpkgs-fmt;
        });
}
