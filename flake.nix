{
  description = "lab: monorepo for capsa, fittings, and friends";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devenv.url = "github:cachix/devenv";
  };

  nixConfig = {
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
    extra-substituters = "https://devenv.cachix.org";
  };

  outputs = { self, nixpkgs, flake-utils, devenv, ... } @ inputs:
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

          # --- shared library discovery ---
          runtimeLibDirs = seedPkg: import ./shared/nix/runtime-lib-dirs.nix {
            inherit lib pkgs seedPkg;
          };

          # --- lockin paths ---
          lockinLibraryDirs = runtimeLibDirs (
            if pkgs.stdenv.isDarwin then pkgs.libiconv
            else pkgs.stdenv.cc.cc
          );

          # --- capsa build infrastructure ---
          capsaPaths = import ./capsa/nix/paths.nix { inherit lib pkgs; };

          capsaPackage = import ./capsa/nix/package.nix {
            inherit lib pkgs capsaPaths;
            src = ./.;
            cargoRoot = "capsa";
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
                    LOCKIN_LIBRARY_DIRS = lockinLibraryDirs;
                    CAPSA_LIBRARY_DIRS = capsaPaths.libraryDirs;
                  } // lib.optionalAttrs (capsaPaths.sydPath != null) {
                    LOCKIN_SYD_PATH = capsaPaths.sydPath;
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

            lockin = devenv.lib.mkShell {
              inherit inputs pkgs;
              modules = [
                ./shared/nix/git-hooks.nix
                ({ lib, pkgs, ... }: {
                  languages.rust.enable = true;

                  env = {
                    LOCKIN_LIBRARY_DIRS = lockinLibraryDirs;
                  } // lib.optionalAttrs pkgs.stdenv.isLinux {
                    LOCKIN_SYD_PATH = "${pkgs.sydbox}/bin/syd";
                  };
                })
              ];
            };

            default = self.devShells.${hostSystem}.capsa;
          };

          formatter = pkgs.nixpkgs-fmt;
        });
}
