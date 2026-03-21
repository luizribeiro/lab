{ lib
, pkgs
, nixpkgs
, hostSystem
, capsaPackage
}:
let
  defaultGuestSystem =
    if lib.hasPrefix "aarch64" hostSystem then
      "aarch64-linux"
    else if lib.hasPrefix "x86_64" hostSystem then
      "x86_64-linux"
    else
      throw "Unsupported host system: ${hostSystem}";

  mkInitramfsLib = guestSystem: import ../initramfs {
    inherit lib;
    pkgs = import nixpkgs { system = guestSystem; };
  };

  defaultGuestModule =
    { modulesPath, ... }:
    {
      imports = [ "${modulesPath}/profiles/minimal.nix" ];

      boot.loader.grub.enable = false;
      boot.loader.systemd-boot.enable = false;

      fileSystems."/" = {
        device = "none";
        fsType = "tmpfs";
      };

      networking.hostName = "capsa";
      system.stateVersion = "25.11";
    };

  mkVMAssets =
    { name ? "capsa"
    , guestSystem ? defaultGuestSystem
    , modules ? [ ]
    , specialArgs ? { }
    , vm ? { }
    ,
    }:
    let
      vmConfig = {
        vcpus = vm.vcpus or 1;
        memoryMiB = vm.memoryMiB or 512;
        kernelCmdline = vm.kernelCmdline or "console=hvc0 rdinit=/init";
        extraArgs = vm.extraArgs or [ ];
      };

      guestNixos = lib.nixosSystem {
        system = guestSystem;
        inherit specialArgs;
        modules = [ defaultGuestModule ] ++ modules;
      };

      kernelImage = "${guestNixos.config.system.build.kernel}/${guestNixos.config.system.boot.loader.kernelFile}";
      initramfsImage = (mkInitramfsLib guestSystem).mkInitramfs {
        inherit (guestNixos.config.system) modulesTree;
      };

      vmAssets = pkgs.runCommand "${name}-vm-assets" { } ''
        mkdir -p $out
        cp ${kernelImage} $out/vmlinuz
        cp ${initramfsImage} $out/initramfs.cpio.lz4
      '';
    in
    {
      inherit name guestSystem vmConfig guestNixos kernelImage initramfsImage vmAssets;
    };

  mkVM = args:
    let
      assets = mkVMAssets args;

      libkrunLibDir =
        if pkgs.stdenv.isDarwin then
          "${lib.getLib pkgs."libkrun-efi"}/lib"
        else
          "${lib.getLib pkgs.libkrun}/lib";

      capsaCmd = "${capsaPackage}/bin/capsa";
    in
    pkgs.writeShellApplication {
      name = "${assets.name}-run";
      text = ''
        export LIBKRUN_LIB_DIR="${libkrunLibDir}"

        exec ${capsaCmd} \
          --kernel ${assets.kernelImage} \
          --initramfs ${assets.initramfsImage} \
          --kernel-cmdline ${lib.escapeShellArg assets.vmConfig.kernelCmdline} \
          --vcpus ${toString assets.vmConfig.vcpus} \
          --memory-mib ${toString assets.vmConfig.memoryMiB} \
          ${lib.escapeShellArgs assets.vmConfig.extraArgs} \
          "$@"
      '';
    };
in
{
  inherit defaultGuestSystem defaultGuestModule mkVMAssets mkVM;
}
