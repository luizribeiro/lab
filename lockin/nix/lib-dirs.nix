{ lib, pkgs, package }:

let
  drv = pkgs.runCommand "${package.name or "pkg"}-lockin-lib-dirs"
    {
      nativeBuildInputs =
        lib.optionals pkgs.stdenv.isDarwin [ pkgs.cctools ]
        ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.glibc.bin ];
    }
    (
      if pkgs.stdenv.isDarwin then ''
        out_file="$out"
        : > "$out_file.tmp"
        if [ -d ${package}/bin ]; then
          for bin in ${package}/bin/*; do
            [ -f "$bin" ] || continue
            otool -L "$bin" 2>/dev/null \
              | grep -oE '/nix/store/[^ ]+\.dylib' \
              | sed 's|/[^/]*$||' >> "$out_file.tmp" || true
          done
        fi
        sort -u "$out_file.tmp" | tr '\n' ':' | sed 's/:$//' > "$out_file"
      ''
      else ''
        out_file="$out"
        : > "$out_file.tmp"
        if [ -d ${package}/bin ]; then
          for bin in ${package}/bin/*; do
            [ -f "$bin" ] || continue
            ldd "$bin" 2>/dev/null \
              | grep -oE '/nix/store/[^ ]+\.so[.0-9]*' \
              | sed 's|/[^/]*$||' >> "$out_file.tmp" || true
          done
        fi
        sort -u "$out_file.tmp" | tr '\n' ':' | sed 's/:$//' > "$out_file"
      ''
    );
in
builtins.readFile drv
