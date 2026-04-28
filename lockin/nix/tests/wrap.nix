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
      filesystem.exec_dirs = [ "/opt/tools" ];
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
      name = "closure: exec_dirs contains hello store path";
      ok = lib.elem helloClosurePath closureMode.filesystem.exec_dirs;
    }
    {
      name = "closure: read_dirs preserves user /etc entry";
      ok = closureMode.filesystem.read_dirs == [ "/etc" ];
    }
    {
      name = "closure: store path not duplicated into read_dirs";
      ok = !(lib.elem helloClosurePath closureMode.filesystem.read_dirs);
    }
    {
      name = "closure: exec_dirs preserves user entry";
      ok = lib.elem "/opt/tools" closureMode.filesystem.exec_dirs;
    }
    {
      name = "closure: user darwin rule preserved (darwin only)";
      ok = !pkgs.stdenv.isDarwin
        || lib.elem ''(allow sysctl-read)'' closureMode.darwin.raw_seatbelt_rules;
    }
    {
      name = "closure: no darwin key on linux";
      ok = pkgs.stdenv.isDarwin || !(closureMode ? darwin);
    }
    {
      name = "full: exec_dirs contains /nix/store";
      ok = lib.elem "/nix/store" fullMode.filesystem.exec_dirs;
    }
    {
      name = "full: no read_dirs key (no user entries)";
      ok = !(fullMode.filesystem ? read_dirs);
    }
    {
      name = "full: no darwin key emitted";
      ok = !(fullMode ? darwin);
    }
    {
      name = "none: read_dirs is exactly user entries";
      ok = noneMode.filesystem.read_dirs == [ "/etc" ];
    }
    {
      name = "none: exec_dirs has no /nix/store closure entries";
      ok =
        let
          dirs = noneMode.filesystem.exec_dirs or [ ];
        in
        !(lib.elem helloClosurePath dirs)
        && !(lib.elem "/nix/store" dirs);
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
