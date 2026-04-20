{ lib, pkgs, lockin }:

{ package
, policy ? { }
, name ? "${package.pname or package.name}-lockin"
, libraryDirs ? null
, extraLibraryDirs ? [ ]
, sydPath ? null
, nixStoreAccess ? "closure"
}:

assert lib.assertOneOf "nixStoreAccess" nixStoreAccess [ "closure" "full" "none" ];

let
  autoDirs =
    if libraryDirs != null then libraryDirs
    else import ./lib-dirs.nix { inherit lib pkgs package; };

  autoDirsList = lib.filter (s: s != "") (lib.splitString ":" autoDirs);

  userLibDirs = policy.filesystem.library_paths or [ ];
  mergedLibDirs = lib.unique (userLibDirs ++ autoDirsList ++ extraLibraryDirs);

  closurePaths =
    let
      info = pkgs.closureInfo { rootPaths = [ package ]; };
      raw = builtins.readFile "${info}/store-paths";
    in
    lib.filter (s: s != "") (lib.splitString "\n" raw);

  storeReadOnlyDirs =
    if nixStoreAccess == "closure" then closurePaths
    else if nixStoreAccess == "full" then [ "/nix/store" ]
    else [ ];

  userReadOnlyDirs = policy.filesystem.read_only_dirs or [ ];
  mergedReadOnlyDirs = lib.unique (userReadOnlyDirs ++ storeReadOnlyDirs);

  userDarwin = policy.darwin or { };
  userDarwinRules = userDarwin.raw_seatbelt_rules or [ ];
  storeSeatbeltRules =
    if !pkgs.stdenv.isDarwin then [ ]
    else if nixStoreAccess == "closure" then
      map (p: ''(allow process-exec (subpath "${p}"))'') closurePaths
    else if nixStoreAccess == "full" then
      [ ''(allow process-exec (subpath "/nix/store"))'' ]
    else [ ];
  mergedDarwinRules = userDarwinRules ++ storeSeatbeltRules;

  basePolicy = builtins.removeAttrs policy [ "filesystem" "darwin" ];
  userFilesystem = policy.filesystem or { };

  filesystemOut = userFilesystem // {
    library_paths = mergedLibDirs;
  } // lib.optionalAttrs (mergedReadOnlyDirs != [ ]) {
    read_only_dirs = mergedReadOnlyDirs;
  };

  darwinOut =
    if pkgs.stdenv.isDarwin then
      userDarwin // lib.optionalAttrs (mergedDarwinRules != [ ]) {
        raw_seatbelt_rules = mergedDarwinRules;
      }
    else { };

  mkConfig = binPath: basePolicy // {
    command = [ binPath ];
    filesystem = filesystemOut;
  } // lib.optionalAttrs (darwinOut != { }) {
    darwin = darwinOut;
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
