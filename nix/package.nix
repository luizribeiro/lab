{ lib
, pkgs
, src
, version ? "0.1.0"
}:
let
  entitlements = src + "/entitlements/capsa.entitlements";
in
pkgs.rustPlatform.buildRustPackage {
  pname = "capsa";
  inherit version src;

  cargoLock = {
    lockFile = src + "/Cargo.lock";
  };

  cargoBuildFlags = [
    "-p"
    "capsa-cli"
    "-p"
    "capsa-vmm"
  ];

  doCheck = false;

  LIBKRUN_LIB_DIR =
    if pkgs.stdenv.isDarwin then
      "${lib.getLib pkgs."libkrun-efi"}/lib"
    else
      "${lib.getLib pkgs.libkrun}/lib";

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

    cat > $out/bin/capsa <<EOF
    #!${pkgs.runtimeShell}
    set -euo pipefail

    export CAPSA_VMM_PATH="$out/libexec/capsa/capsa-vmm"
    exec "$out/libexec/capsa/capsa" "\$@"
    EOF

    chmod +x $out/bin/capsa
  '';

  postFixup = lib.optionalString pkgs.stdenv.isDarwin ''
    /usr/bin/codesign --force --sign - --entitlements ${entitlements} $out/libexec/capsa/capsa
    /usr/bin/codesign --force --sign - --entitlements ${entitlements} $out/libexec/capsa/capsa-vmm
  '';
}
