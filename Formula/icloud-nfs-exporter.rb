class IcloudNfsExporter < Formula
  desc "Export iCloud Drive folders via NFS with transparent file hydration"
  homepage "https://github.com/wizz-cmd/icloud-nfs-exporter"
  url "https://github.com/wizz-cmd/icloud-nfs-exporter/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER"
  license "MIT"
  head "https://github.com/wizz-cmd/icloud-nfs-exporter.git", branch: "main"

  depends_on :macos
  depends_on xcode: ["15.0", :build]
  depends_on "rust" => :build
  depends_on "python@3.11"

  def install
    # Swift packages
    system "swift", "build", "--package-path", "src/hydration",
           "-c", "release", "--arch", "arm64", "--arch", "x86_64"
    system "swift", "build", "--package-path", "src/app",
           "-c", "release", "--arch", "arm64", "--arch", "x86_64"

    bin.install "src/hydration/.build/apple/Products/Release/HydrationDaemon"
    bin.install "src/app/.build/apple/Products/Release/MenuBarApp" => "icloud-nfs-exporter-app"

    # Rust workspace
    cd "src/fuse" do
      system "cargo", "build", "--release"
      bin.install "target/release/fuse-driver"
    end

    # CLI scripts
    libexec.install "scripts/icne"
    (libexec/"icne_lib").install Dir["scripts/icne_lib/*.py"]
    bin.install_symlink libexec/"icne"

    # Launchd template
    share.install "launchd/com.wizz-cmd.icloud-nfs-exporter.plist.template"
  end

  def caveats
    <<~EOS
      To get started:
        icne setup

      macFUSE is required for the FUSE passthrough driver:
        https://osxfuse.github.io/

      The hydration daemon can be started as a LaunchAgent:
        icne setup  # installs the LaunchAgent automatically
    EOS
  end

  service do
    run opt_bin/"HydrationDaemon"
    keep_alive true
    log_path var/"log/icloud-nfs-exporter.log"
    error_log_path var/"log/icloud-nfs-exporter.err"
  end

  test do
    assert_match "0.1.0", shell_output("#{bin}/HydrationDaemon --version")
    assert_match "icne", shell_output("#{bin}/icne --help")
  end
end
