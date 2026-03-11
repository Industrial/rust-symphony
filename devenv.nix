{
  inputs,
  pkgs,
  ...
}: let
  pkgs-playwright = import inputs.nixpkgs-playwright {system = pkgs.stdenv.hostPlatform.system;};
  browsers = (builtins.fromJSON (builtins.readFile "${pkgs-playwright.playwright-driver}/browsers.json")).browsers;
  chromium-rev = (builtins.head (builtins.filter (x: x.name == "chromium") browsers)).revision;

  # Moon from GitHub releases (x86_64-linux). See https://moonrepo.dev/docs/install
  moon-version = "v2.0.4";
  moon = pkgs.stdenv.mkDerivation {
    pname = "moon-cli";
    version = builtins.replaceStrings ["v"] [""] moon-version;
    src = pkgs.fetchurl {
      url = "https://github.com/moonrepo/moon/releases/download/${moon-version}/moon_cli-x86_64-unknown-linux-gnu.tar.xz";
      sha256 = "0n7w3pmnwaxk0cy63ms97g609z696698a4qdrssnsa7cs8wgxxc8";
    };
    nativeBuildInputs = [pkgs.autoPatchelfHook];
    buildInputs = [pkgs.stdenv.cc.cc.lib];
    installPhase = ''
      runHook preInstall
      mkdir -p $out/bin
      install -m755 moon $out/bin/moon
      runHook postInstall
    '';
    meta = {
      description = "Moon CLI (moonrepo)";
      homepage = "https://moonrepo.dev";
      license = pkgs.lib.licenses.mit;
      platforms = pkgs.lib.platforms.linux;
    };
  };
in {
  # Name of the project with version
  name = "rust-symphony";

  # Languages
  languages = {
    javascript = {
      enable = true;
      package = pkgs.nodejs_22;
      bun = {
        enable = true;
      };
    };

    typescript = {
      enable = true;
    };

    rust = {
      enable = true;
      channel = "stable";
      components = [
        "cargo"
        "clippy"
        "rust-analyzer"
        "rustc"
        "rustfmt"
        "llvm-tools"
      ];
      targets = [];
    };
  };

  env = {
    LD_LIBRARY_PATH = builtins.concatStringsSep ":" [
      "${pkgs.stdenv.cc.cc.lib}/lib"
      "${pkgs.vips}/lib"
      "${pkgs.openssl.out}/lib"
      "${pkgs.glib.out}/lib"
      "${pkgs.nss.out}/lib"
      "${pkgs.nspr.out}/lib"
      "${pkgs.dbus.lib}/lib"
      "${pkgs.atk.out}/lib"
      "${pkgs.at-spi2-atk.out}/lib"
      "${pkgs.expat.out}/lib"
      "${pkgs.at-spi2-core.out}/lib"
      "${pkgs.xorg.libX11.out}/lib"
      "${pkgs.xorg.libXcomposite.out}/lib"
      "${pkgs.xorg.libXdamage.out}/lib"
      "${pkgs.xorg.libXext.out}/lib"
      "${pkgs.xorg.libXfixes.out}/lib"
      "${pkgs.xorg.libXrandr.out}/lib"
      "${pkgs.mesa.out}/lib"
      "${pkgs.xorg.libxcb.out}/lib"
      "${pkgs.libxkbcommon.out}/lib"
      "${pkgs.systemd}/lib"
      "${pkgs.alsa-lib.out}/lib"
    ];
    PKG_CONFIG_PATH = "${pkgs.vips}/lib/pkgconfig:${pkgs.pkg-config}/lib/pkgconfig:${pkgs.openssl.out}/lib/pkgconfig";
    # Playwright browsers are provided via environment variables from pinned nixpkgs-playwright
    PLAYWRIGHT_BROWSERS_PATH = "${pkgs-playwright.playwright.browsers}";
    PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = true;
    PLAYWRIGHT_NODEJS_PATH = "${pkgs.nodejs_22}/bin/node";
    PLAYWRIGHT_LAUNCH_OPTIONS_EXECUTABLE_PATH = "${pkgs-playwright.playwright.browsers}/chromium-${chromium-rev}/chrome-linux/chrome";

    RUST_BACKTRACE = "1";
    CARGO_TERM_COLOR = "always";
    # Enable SQL statement logging (rust-symphony db layer). SQL appears when RUST_SYMPHONY_SQL_DEBUG=1 and sqlx=debug below.
    RUST_SYMPHONY_SQL_DEBUG = "1";
    # Show SQL queries (sqlx) and app/rust-symphony at debug. Omit sqlx=debug to disable SQL logging.
    RUST_LOG = "info,rust_symphony=debug,app=trace,sqlx=info";

    # Build optimization: sccache for compilation caching
    # Uses default $HOME/.cache/sccache location (no custom wrapper needed)
    RUSTC_WRAPPER = "sccache";

    # Moon: Use system Rust instead of installing via proto/rustup
    MOON_TOOLCHAIN_FORCE_GLOBALS = "rust";
  };

  # Development packages
  packages = with pkgs; [
    # AI
    inputs.nixpkgs-unstable.legacyPackages.${stdenv.hostPlatform.system}.beads

    # E2E browser automation (fantoccini + chromedriver)
    chromedriver
    chromium

    # Rust tools
    clippy
    rust-analyzer
    rustc

    # Development tools
    direnv
    # Git hooks (prek = pre-commit replacement, single binary, no Python)
    prek

    # Formatting tools
    alejandra

    # Publishing tools
    cargo-watch
    cargo-audit
    cargo-llvm-cov
    cargo-nextest

    # Build optimization
    sccache
    mold

    # Version management
    git
    gh

    # Build system
    moon

    # treefmt
    actionlint
    alejandra
    beautysh
    biome
    deadnix
    rustfmt
    taplo
    treefmt
    vulnix
    yamlfmt
  ];

  scripts = {
    intro = {
      exec = ''
        # Check installed version from package.json in the project root
        playwrightInstalledVersion=""
        if [ -f ./package.json ]; then
          playwrightInstalledVersion=$(grep -o '"@playwright/test":\s*"[^"]*"' ./package.json 2>/dev/null | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' | head -1)
        fi

        if [ -z "$playwrightInstalledVersion" ]; then
          playwrightInstalledVersion="not found"
        fi

        echo "❄️  Playwright nix version: ${pkgs-playwright.playwright.version}"
        echo "📦 Playwright bun version: $playwrightInstalledVersion"

        if [ "$playwrightInstalledVersion" != "not found" ] && [ "${pkgs-playwright.playwright.version}" != "$playwrightInstalledVersion" ]; then
          echo "⚠️  Playwright versions in nix (in devenv.yaml) and bun (in package.json) are not the same! Please adapt the configuration."
        else
          echo "✅ Playwright versions in nix and bun are compatible"
        fi

        echo
        echo "Environment variables:"
        env | grep ^PLAYWRIGHT
      '';
    };

    prek-install = {
      exec = ''
        prek install -q --overwrite
      '';
    };

    moon-sync = {
      exec = ''
        moon sync
      '';
    };
  };

  enterShell = ''
    prek-install
    intro
    moon-sync

    # Ensure sccache default cache directory exists and is writable
    # sccache uses $HOME/.cache/sccache by default (no wrapper needed)
    mkdir -p "$HOME/.cache/sccache"
    chmod 755 "$HOME/.cache/sccache" 2>/dev/null || true

    # Note: sccache will automatically start its server when first used
    # RUSTC_WRAPPER is already set to "sccache" in env block above

    # Add rust-symphony CLI to PATH if it exists, prioritizing debug during dev
    if [ -f ./target/debug/rust-symphony ] && [ -f ./target/release/rust-symphony ]; then
      if [ ./target/debug/rust-symphony -nt ./target/release/rust-symphony ]; then
        export PATH="$PWD/target/debug:$PATH"
        echo "rust-symphony CLI (debug) available in PATH"
      else
        export PATH="$PWD/target/release:$PATH"
        echo "rust-symphony CLI (release) available in PATH"
      fi
    elif [ -f ./target/debug/rust-symphony ]; then
      export PATH="$PWD/target/debug:$PATH"
      echo "rust-symphony CLI (debug) available in PATH"
    elif [ -f ./target/release/rust-symphony ]; then
      export PATH="$PWD/target/release:$PATH"
      echo "rust-symphony CLI (release) available in PATH"
    fi
  '';
}
