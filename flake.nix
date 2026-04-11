{
  description = "capsa: Rust + libkrun dev shell and minimal VM kernel/initramfs assets";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    git-hooks.url = "github:cachix/git-hooks.nix";
  };

  outputs = { self, nixpkgs, flake-utils, git-hooks, ... }:
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
                  # Nixpkgs builds libkrun with networking disabled by default.
                  # Enable NET=1 so krun_add_net_unixstream is available.
                  libkrun = prev.libkrun.override {
                    withNet = true;
                  };
                })
            ];
          };

          capsaPaths = import ./nix/paths.nix { inherit lib pkgs; };

          capsaPackage = import ./nix/package.nix {
            inherit lib pkgs capsaPaths;
            src = ./.;
          };

          vmLib = import ./nix/vm {
            inherit lib pkgs nixpkgs hostSystem capsaPackage capsaPaths;
          };

          defaultVmAssets = vmLib.mkVMAssets { name = "capsa"; };
          defaultVm = vmLib.mkVM { name = "capsa"; };

          preCommitCheck = git-hooks.lib.${hostSystem}.run {
            src = ./.;
            hooks = {
              rustfmt.enable = true;
              clippy = {
                enable = true;
                packageOverrides = {
                  inherit (pkgs) cargo clippy;
                };
                settings = {
                  denyWarnings = true;
                  extraArgs = "--target-dir target/pre-commit";
                };
              };
              nixpkgs-fmt.enable = true;
              statix.enable = true;
              deadnix.enable = true;
            };
          };

          checks = import ./nix/checks {
            inherit vmLib pkgs capsaPaths;
          };
        in
        {
          lib = {
            inherit (vmLib) mkVMAssets mkVM mkVMCheck;
          };

          packages = {
            capsa = capsaPackage;

            vm-assets = defaultVmAssets.vmAssets;

            vm = defaultVm;

            default = self.packages.${hostSystem}.capsa;
          };

          inherit checks;

          devShells.default = pkgs.mkShell {
            packages =
              (with pkgs; [
                cargo
                rustc
                rustfmt
                clippy
                pkg-config
              ])
              ++ preCommitCheck.enabledPackages
              ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.libkrun pkgs.sydbox ]
              ++ lib.optionals pkgs.stdenv.isDarwin [
                pkgs."libkrun-efi"
                pkgs.libepoxy
                pkgs.virglrenderer
              ];

            shellHook = lib.concatStringsSep "\n" [
              preCommitCheck.shellHook
              ''
                export LIBKRUN_LIB_DIR="''${LIBKRUN_LIB_DIR:-${capsaPaths.libkrunLibDir}}"
                export CAPSA_LIBRARY_DIRS="''${CAPSA_LIBRARY_DIRS:-${capsaPaths.libraryDirs}}"
              ''
              (lib.optionalString (capsaPaths.sydPath != null) ''
                export CAPSA_SYD_PATH="''${CAPSA_SYD_PATH:-${capsaPaths.sydPath}}"
              '')
            ];
          };

          formatter = pkgs.nixpkgs-fmt;
        });
}
