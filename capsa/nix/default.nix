{ lib, pkgs, nixpkgs, hostSystem }:

let
  capsaPaths = import ./paths.nix { inherit lib pkgs; };

  capsaPackage = import ./package.nix {
    inherit lib pkgs capsaPaths;
    src = ../..;
  };

  vmLib = import ./vm {
    inherit lib pkgs nixpkgs hostSystem capsaPackage capsaPaths;
  };
in
{
  packages = {
    capsa = capsaPackage;
    vm-assets = (vmLib.mkVMAssets { name = "capsa"; }).vmAssets;
    vm = vmLib.mkVM { name = "capsa"; };
  };

  checks = import ./checks {
    inherit vmLib pkgs capsaPaths capsaPackage;
  };

  lib = {
    inherit (vmLib) mkVMAssets mkVM mkVMCheck;
  };
}
