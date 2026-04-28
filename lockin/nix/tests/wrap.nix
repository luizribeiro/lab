{ lib, pkgs, wrapWithLockin }:

let
  readConfig = wrapped:
    let
      binDir = "${wrapped}/bin";
      firstBin = lib.head (builtins.attrNames (builtins.readDir binDir));
      script = builtins.readFile "${binDir}/${firstBin}";
      match = builtins.match ".*-c (/nix/store/[^ ]+\\.toml).*" script;
      tomlPath = lib.head match;
    in
    builtins.fromTOML (builtins.unsafeDiscardStringContext (builtins.readFile tomlPath));

  closureMode = readConfig (wrapWithLockin {
    package = pkgs.hello;
    policy = {
      filesystem.read_dirs = [ "/etc" ];
      darwin.raw_seatbelt_rules = [ ''(allow sysctl-read)'' ];
    };
  });

  fullMode = readConfig (wrapWithLockin {
    package = pkgs.hello;
    nixStoreAccess = "full";
  });

  noneMode = readConfig (wrapWithLockin {
    package = pkgs.hello;
    nixStoreAccess = "none";
    policy.filesystem.read_dirs = [ "/etc" ];
  });

  helloClosurePath = builtins.unsafeDiscardStringContext "${pkgs.hello}";

  assertions = [
    {
      name = "closure: read_dirs contains hello store path";
      ok = lib.elem helloClosurePath closureMode.filesystem.read_dirs;
    }
    {
      name = "closure: read_dirs preserves user /etc entry";
      ok = lib.elem "/etc" closureMode.filesystem.read_dirs;
    }
    {
      name = "closure: user darwin rule preserved (darwin only)";
      ok = !pkgs.stdenv.isDarwin
        || lib.elem ''(allow sysctl-read)'' closureMode.darwin.raw_seatbelt_rules;
    }
    {
      name = "closure: generated seatbelt rule emitted (darwin only)";
      ok = !pkgs.stdenv.isDarwin
        || lib.any (r: lib.hasInfix helloClosurePath r) closureMode.darwin.raw_seatbelt_rules;
    }
    {
      name = "closure: no darwin key on linux";
      ok = pkgs.stdenv.isDarwin || !(closureMode ? darwin);
    }
    {
      name = "full: read_dirs is exactly /nix/store";
      ok = fullMode.filesystem.read_dirs == [ "/nix/store" ];
    }
    {
      name = "full: single seatbelt rule granting /nix/store (darwin only)";
      ok = !pkgs.stdenv.isDarwin
        || fullMode.darwin.raw_seatbelt_rules == [ ''(allow process-exec (subpath "/nix/store"))'' ];
    }
    {
      name = "none: read_dirs is exactly user entries";
      ok = noneMode.filesystem.read_dirs == [ "/etc" ];
    }
    {
      name = "none: no darwin key emitted";
      ok = !(noneMode ? darwin);
    }
  ];

  failed = lib.filter (a: !a.ok) assertions;
in
if failed == [ ] then
  pkgs.runCommand "lockin-wrap-tests-ok" { } "echo ok > $out"
else
  throw "lockin wrap tests failed:\n${lib.concatMapStringsSep "\n" (a: "  - ${a.name}") failed}"
