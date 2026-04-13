{ lib, pkgs, seedPkg }:

let
  runtimeLibraryDirs = pkgs.runCommand "runtime-library-dirs"
    {
      nativeBuildInputs =
        lib.optionals pkgs.stdenv.isDarwin [ pkgs.cctools ]
        ++ lib.optionals pkgs.stdenv.isLinux [ pkgs.glibc.bin ];
    }
    (
      if pkgs.stdenv.isDarwin then ''
        seen="$(mktemp)"
        queue="$(mktemp)"
        next_queue="$(mktemp)"

        echo "${lib.getLib seedPkg}/lib" > "$seen"
        find ${lib.getLib seedPkg}/lib -name '*.dylib' > "$queue"

        while [ -s "$queue" ]; do
          > "$next_queue"
          while IFS= read -r dylib; do
            deps=$(otool -L "$dylib" 2>/dev/null \
              | grep -oE '/nix/store/[^ ]+\.dylib' || true)
            while IFS= read -r dep; do
              [ -z "$dep" ] && continue
              dir="$(dirname "$dep")"
              if ! grep -qxF "$dir" "$seen"; then
                echo "$dir" >> "$seen"
                echo "$dep" >> "$next_queue"
              fi
            done <<< "$deps"
          done < "$queue"
          cp "$next_queue" "$queue"
        done

        sort -u "$seen" | tr '\n' ':' | sed 's/:$//' > $out
      ''
      else ''
        {
          find ${lib.getLib seedPkg}/lib -name '*.so*' -exec ldd {} + 2>/dev/null \
            | grep -oE '/nix/store/[^ ]+\.so[.0-9]*' \
            | sed 's|/[^/]*$||' \
            || true

          echo "${lib.getLib pkgs.glibc}/lib"
          echo "${lib.getLib pkgs.stdenv.cc.cc}/lib"
          ${lib.optionalString (pkgs.stdenv.cc.cc ? libgcc)
            ''echo "${pkgs.stdenv.cc.cc.libgcc}/lib"''}
        } | sort -u | tr '\n' ':' | sed 's/:$//' > $out
      ''
    );
  nixDirs = builtins.readFile runtimeLibraryDirs;

  # System library dirs so sandboxed system binaries (e.g.
  # /usr/bin/env) can load their dynamic libraries under syd.
  systemDirs = lib.optionalString pkgs.stdenv.isLinux
    (lib.concatStringsSep ":" [
      "/lib"
      "/lib64"
      "/usr/lib"
      "/usr/lib64"
      "/lib/x86_64-linux-gnu"
      "/usr/lib/x86_64-linux-gnu"
      "/lib/aarch64-linux-gnu"
      "/usr/lib/aarch64-linux-gnu"
    ]);
in
if systemDirs == "" then nixDirs
else "${nixDirs}:${systemDirs}"
