class Sshr < Formula
  desc "A TUI for managing and connecting to SSH hosts"
  homepage "https://github.com/hoangneeee/sshr"
  url "https://github.com/hoangneeee/sshr/releases/download/v0.1.0/sshr-x86_64-apple-darwin.tar.gz"
  sha256 "cef9f57f72a2b046d6e82089f6d280748dc53241157383563882e9e8c646baee" # "shasum -a 256 <SHA256 của file tar.gz>"
  license "Apache-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/sshr", "--version"
  end
end