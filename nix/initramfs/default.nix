{ lib, pkgs }:

let
  busyboxPackage =
    if pkgs.stdenv.hostPlatform.isLinux then
      pkgs.pkgsStatic.busybox
    else
      pkgs.busybox;

  kmodPackage =
    if pkgs.stdenv.hostPlatform.isLinux then
      pkgs.pkgsStatic.kmod
    else
      pkgs.kmod;
in
{
  mkInitramfs =
    { modulesTree ? null }:
    pkgs.runCommand "capsa-initramfs.cpio.lz4"
      {
        nativeBuildInputs = [
          pkgs.cpio
          pkgs.findutils
          pkgs.lz4
        ];
      } ''
      set -euo pipefail

      root="$TMPDIR/initramfs-root"
      mkdir -p "$root"/{bin,sbin,proc,sys,dev,run}

      cp ${busyboxPackage}/bin/busybox "$root/bin/busybox"
      chmod 0755 "$root/bin/busybox"

      ${lib.optionalString (modulesTree != null) ''
        if [ -d "${modulesTree}/lib/modules" ]; then
          mkdir -p "$root/lib"
          cp -a "${modulesTree}/lib/modules" "$root/lib/modules"
        fi
      ''}

      for cmd in \
        sh mount umount mkdir sleep dmesg switch_root \
        cat echo ls test readlink poweroff reboot setsid cttyhack; do
        ln -s busybox "$root/bin/$cmd"
      done

      cp ${kmodPackage}/bin/modprobe "$root/bin/modprobe"
      chmod 0755 "$root/bin/modprobe"

      cp ${./init.sh} "$root/init"
      chmod 0755 "$root/init"

      (
        cd "$root"
        find . -mindepth 1 -print | sort | cpio --quiet -o -H newc
      ) | lz4 -l -9 > "$out"
    '';
}
