{ lib
, pkgs
, capsaPaths
, src
, cargoRoot ? null
, version ? "0.1.0"
}:
let
  prefix = if cargoRoot != null then cargoRoot + "/" else "";
  entitlements = src + "/${prefix}entitlements/capsa.entitlements";
in
pkgs.rustPlatform.buildRustPackage ({
  pname = "capsa";
  inherit version src;

  cargoLock = {
    lockFile = src + "/${prefix}Cargo.lock";
  };

  cargoBuildFlags = [
    "-p"
    "capsa-cli"
    "-p"
    "capsa-vmm"
    "-p"
    "capsa-netd"
  ];

  doCheck = false;

  LIBKRUN_LIB_DIR = capsaPaths.libkrunLibDir;

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs =
    lib.optionals pkgs.stdenv.isLinux [ pkgs.libkrun ]
      ++ lib.optionals pkgs.stdenv.isDarwin [
      pkgs."libkrun-efi"
      pkgs.libepoxy
      pkgs.virglrenderer
    ];

  installPhase = ''
    mkdir -p $out/bin $out/libexec/capsa

    release_dir="target/release"
    if [ ! -x "$release_dir/capsa" ]; then
      for d in target/*/release; do
        if [ -x "$d/capsa" ]; then
          release_dir="$d"
          break
        fi
      done
    fi

    cp "$release_dir/capsa" $out/libexec/capsa/capsa
    cp "$release_dir/capsa-vmm" $out/libexec/capsa/capsa-vmm
    cp "$release_dir/capsa-netd" $out/libexec/capsa/capsa-netd

    cat > $out/bin/capsa <<EOF
    #!${pkgs.runtimeShell}
    set -euo pipefail

    export CAPSA_VMM_PATH="$out/libexec/capsa/capsa-vmm"
    export CAPSA_NETD_PATH="$out/libexec/capsa/capsa-netd"
    export CAPSA_LIBRARY_DIRS="${capsaPaths.libraryDirs}"
    ${lib.optionalString (capsaPaths.sydPath != null) ''
    export CAPSA_SYD_PATH="${capsaPaths.sydPath}"
    ''}
    exec "$out/libexec/capsa/capsa" "\$@"
    EOF

    chmod +x $out/bin/capsa
  '';

  postFixup = lib.optionalString pkgs.stdenv.isDarwin ''
    /usr/bin/codesign --force --sign - $out/libexec/capsa/capsa
    /usr/bin/codesign --force --sign - $out/libexec/capsa/capsa-netd
    /usr/bin/codesign --force --sign - --entitlements ${entitlements} $out/libexec/capsa/capsa-vmm
  '';
} // lib.optionalAttrs (cargoRoot != null) {
  inherit cargoRoot;
  preBuild = "cd ${cargoRoot}";
})
