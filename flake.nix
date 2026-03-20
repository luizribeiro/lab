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
          pkgs = import nixpkgs { system = hostSystem; };

          guestSystem =
            if lib.hasPrefix "aarch64" hostSystem then
              "aarch64-linux"
            else if lib.hasPrefix "x86_64" hostSystem then
              "x86_64-linux"
            else
              throw "Unsupported host system: ${hostSystem}";

          initramfsLib = import ./nix/initramfs {
            inherit lib;
            pkgs = import nixpkgs { system = guestSystem; };
          };

          guestNixos = lib.nixosSystem {
            system = guestSystem;
            modules = [
              ({ modulesPath, ... }: {
                imports = [ "${modulesPath}/profiles/minimal.nix" ];

                boot.loader.grub.enable = false;
                boot.loader.systemd-boot.enable = false;

                fileSystems."/" = {
                  device = "none";
                  fsType = "tmpfs";
                };

                networking.hostName = "capsa";
                system.stateVersion = "25.11";
              })
            ];
          };

          kernelImage = "${guestNixos.config.system.build.kernel}/${guestNixos.config.system.boot.loader.kernelFile}";
          initramfsImage = initramfsLib.mkInitramfs {
            inherit (guestNixos.config.system) modulesTree;
          };

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
        in
        {
          packages = {
            vm-kernel = pkgs.runCommand "capsa-vmlinuz" { } ''
              cp ${kernelImage} $out
            '';

            vm-initramfs = initramfsImage;

            vm-assets = pkgs.runCommand "capsa-vm-assets" { } ''
              mkdir -p $out
              cp ${kernelImage} $out/vmlinuz
              cp ${initramfsImage} $out/initramfs.cpio.lz4
            '';

            default = self.packages.${hostSystem}.vm-assets;
          };

          checks = {
            pre-commit-check = preCommitCheck;
          };

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
              # Linux links directly against nixpkgs libkrun in the dev shell.
              ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.libkrun ]
              # Darwin uses krunkit's libkrun-efi runtime and graphics deps.
              ++ lib.optionals pkgs.stdenv.isDarwin [
                pkgs.krunkit
                pkgs.libepoxy
                pkgs.virglrenderer
              ];

            shellHook = lib.concatStringsSep "\n" [
              preCommitCheck.shellHook
              (lib.optionalString pkgs.stdenv.isDarwin ''
                _capsa_krun_efi_store_path="$(nix-store -q --references ${pkgs.krunkit} | grep 'libkrun-efi-' | head -n1)"
                if [ -n "$_capsa_krun_efi_store_path" ]; then
                  _capsa_krun_efi_libdir="$_capsa_krun_efi_store_path/lib"
                  _capsa_krun_efi_dylib="$(ls "$_capsa_krun_efi_libdir"/libkrun-efi*.dylib 2>/dev/null | head -n1)"

                  if [ -n "$_capsa_krun_efi_dylib" ] && [ -z "''${CAPSA_LIBKRUN_DYLIB:-}" ]; then
                    export CAPSA_LIBKRUN_DYLIB="$_capsa_krun_efi_dylib"
                  fi

                  export DYLD_LIBRARY_PATH="$_capsa_krun_efi_libdir:''${DYLD_LIBRARY_PATH:-}"
                  export LIBRARY_PATH="$_capsa_krun_efi_libdir:''${LIBRARY_PATH:-}"
                  export RUSTFLAGS="-L native=$_capsa_krun_efi_libdir -C link-arg=-Wl,-rpath,$_capsa_krun_efi_libdir ''${RUSTFLAGS:-}"
                fi
                unset _capsa_krun_efi_store_path _capsa_krun_efi_libdir _capsa_krun_efi_dylib
              '')
            ];
          };

          formatter = pkgs.nixpkgs-fmt;
        });
}
