{ lib
, pkgs
, src
, version ? "0.1.0"
}:
let
  entitlements = src + "/entitlements/capsa.entitlements";

  libraryDirs =
    if pkgs.stdenv.isDarwin then
      lib.concatStringsSep ":" [
        "${lib.getLib pkgs.libiconv}/lib"
        "${lib.getLib pkgs."libkrun-efi"}/lib"
      ]
    else
      lib.concatStringsSep ":" [
        "${lib.getLib pkgs.glibc}/lib"
        "${lib.getLib pkgs.stdenv.cc.cc}/lib"
        "${pkgs.stdenv.cc.cc.libgcc}/lib"
        "${lib.getLib pkgs.libkrun}/lib"
      ];
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
    "-p"
    "capsa-netd"
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
    cp "$release_dir/capsa-netd" $out/libexec/capsa/capsa-netd

    cat > $out/bin/capsa <<EOF
    #!${pkgs.runtimeShell}
    set -euo pipefail

    export CAPSA_VMM_PATH="$out/libexec/capsa/capsa-vmm"
    export CAPSA_NETD_PATH="$out/libexec/capsa/capsa-netd"
    export CAPSA_LIBRARY_DIRS="${libraryDirs}"
    ${lib.optionalString pkgs.stdenv.isLinux ''
    export CAPSA_SYD_PATH="${pkgs.sydbox}/bin/syd"
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
}
