class Rafaello < Formula
  desc "v1 demo-ready CLI for the rafaello agent"
  homepage "https://github.com/luizribeiro/lab"
  version "<populated-by-G2-on-release>"

  on_arm do
    on_linux do
      url "<aarch64-linux tarball URL>"
      sha256 "<aarch64-linux sha>"
    end
    on_macos do
      url "<aarch64-darwin tarball URL>"
      sha256 "<aarch64-darwin sha>"
    end
  end

  on_intel do
    on_linux do
      url "<x86_64-linux tarball URL>"
      sha256 "<x86_64-linux sha>"
    end
  end

  def install
    bin.install "bin/rfl"
    bin.install "bin/rfl-tui"
    (share/"rafaello/plugins").install Dir["share/rafaello/plugins/*"]
  end

  test do
    system bin/"rfl", "--version"
  end
end
