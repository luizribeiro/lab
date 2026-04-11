{ lib, pkgs }:
let
  libkrunPkg =
    if pkgs.stdenv.isDarwin then pkgs."libkrun-efi"
    else pkgs.libkrun;

  # Discovers Nix-store library directories needed at runtime by
  # walking the linked dependency graph of libkrun.  Uses IFD
  # (builtins.readFile on a derivation) so the result is always in
  # sync with the actual binary — at the cost of triggering a build
  # during Nix evaluation.
  runtimeLibraryDirs = pkgs.runCommand "capsa-runtime-library-dirs"
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

        echo "${lib.getLib libkrunPkg}/lib" > "$seen"
        find ${lib.getLib libkrunPkg}/lib -name '*.dylib' > "$queue"

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
          find ${lib.getLib libkrunPkg}/lib -name '*.so*' -exec ldd {} + 2>/dev/null \
            | grep -oE '/nix/store/[^ ]+\.so[.0-9]*' \
            | sed 's|/[^/]*$||'

          echo "${lib.getLib pkgs.glibc}/lib"
          echo "${lib.getLib pkgs.stdenv.cc.cc}/lib"
          ${lib.optionalString (pkgs.stdenv.cc.cc ? libgcc)
            ''echo "${pkgs.stdenv.cc.cc.libgcc}/lib"''}
        } | sort -u | tr '\n' ':' | sed 's/:$//' > $out
      ''
    );

  libraryDirs = builtins.readFile runtimeLibraryDirs;
in
rec {
  libkrunLibDir = "${lib.getLib libkrunPkg}/lib";

  inherit libraryDirs;

  sydPath =
    if pkgs.stdenv.isLinux then "${pkgs.sydbox}/bin/syd"
    else null;
}
