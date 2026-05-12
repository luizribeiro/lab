{ pkgs
, src
, version ? "0.0.0"
}:
pkgs.rustPlatform.buildRustPackage {
  pname = "rafaello";
  inherit version src;

  cargoRoot = "rafaello";
  buildAndTestSubdir = "rafaello";

  cargoLock = {
    lockFile = src + "/rafaello/Cargo.lock";
  };

  cargoBuildFlags = [
    "-p"
    "rafaello"
    "-p"
    "rafaello-tui"
    "-p"
    "rafaello-openai"
    "-p"
    "rafaello-openai-stub"
    "-p"
    "rafaello-readfile"
    "-p"
    "rafaello-mailcat"
    "-p"
    "rafaello-mockprovider"
    "-p"
    "rafaello-fetch"
  ];

  doCheck = false;

  postInstall = ''
    plugins=(
      "rfl-mailcat:rafaello-mailcat"
      "rfl-readfile:rafaello-readfile"
      "rfl-openai:rafaello-openai"
      "rfl-openai-stub:rafaello-openai-stub"
      "rfl-mockprovider:rafaello-mockprovider"
      "rafaello-fetch:rafaello-fetch"
    )
    for entry in "''${plugins[@]}"; do
      name="''${entry%%:*}"
      crate="''${entry##*:}"
      src_dir="rafaello/crates/$crate"
      dst_dir="$out/share/rafaello/plugins/$name"
      mkdir -p "$dst_dir/bin"
      cp "$src_dir/rafaello.toml" "$dst_dir/rafaello.toml"
      cp "$src_dir/openrpc.json" "$dst_dir/openrpc.json"
      if [ -d "$src_dir/schemas" ]; then
        cp -r "$src_dir/schemas" "$dst_dir/schemas"
      fi
      bin_src="$out/bin/$name"
      bin_dst="$dst_dir/bin/$name"
      cp --remove-destination "$bin_src" "$bin_dst"
      chmod +x "$bin_dst"
      rm "$bin_src"
    done
  '';
}
