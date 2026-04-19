{ lib, pkgs, lockin }:

{ package
, policy ? { }
, name ? "${package.pname or package.name}-lockin"
, libraryDirs ? null
, extraLibraryDirs ? [ ]
, sydPath ? null
}:

let
  autoDirs =
    if libraryDirs != null then libraryDirs
    else import ./lib-dirs.nix { inherit lib pkgs package; };

  autoDirsList = lib.filter (s: s != "") (lib.splitString ":" autoDirs);

  userLibDirs = policy.filesystem.library_paths or [ ];
  mergedLibDirs = lib.unique (userLibDirs ++ autoDirsList ++ extraLibraryDirs);

  basePolicy = builtins.removeAttrs policy [ "filesystem" ];
  userFilesystem = policy.filesystem or { };

  mkConfig = binPath: basePolicy // {
    command = [ binPath ];
    filesystem = userFilesystem // {
      library_paths = mergedLibDirs;
    };
  };

  tomlFormat = pkgs.formats.toml { };

  resolvedSyd =
    if sydPath != null then sydPath
    else if pkgs.stdenv.isLinux then "${lib.getBin pkgs.sydbox}/bin/syd"
    else null;

  bins =
    let entries = builtins.readDir "${package}/bin";
    in lib.mapAttrsToList (n: _: { name = n; path = "${package}/bin/${n}"; })
      (lib.filterAttrs (_: t: t == "regular" || t == "symlink") entries);

  mkWrapperLine = bin: ''
    makeWrapper ${lockin}/bin/lockin $out/bin/${bin.name} \
      --add-flags "-c ${tomlFormat.generate "${name}-${bin.name}.toml" (mkConfig bin.path)} --"${
        lib.optionalString (resolvedSyd != null)
          " \\\n      --set LOCKIN_SYD_PATH ${resolvedSyd}"
      }
  '';
in
pkgs.runCommand name
{
  nativeBuildInputs = [ pkgs.makeWrapper ];
  passthru = { inherit package policy; };
}
  ''
    mkdir -p $out/bin
    ${lib.concatMapStringsSep "\n" mkWrapperLine bins}
  ''
