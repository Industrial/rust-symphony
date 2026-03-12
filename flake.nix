{
  description = "Attractor pipeline as a StreamWeave graph";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix/release-0.12";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    cargo2nix,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo2nix.overlays.default];
      };
      # cargo2nix build (optional): only when Cargo.nix exists.
      # Generate with: nix run .#generate  (or nix run github:cargo2nix/cargo2nix -- cargo2nix)
      hasCargoNix = pkgs.lib.pathExists (self + "/Cargo.nix");
      rustPkgs =
        if hasCargoNix
        then
          pkgs.rustBuilder.makePackageSet {
            packageFun = import (self + "/Cargo.nix");
            workspaceSrc = self;
            rustVersion = "1.83.0";
            packageOverrides = pkgs: pkgs.rustBuilder.overrides.all;
          }
        else null;
      cargo2nixPackage =
        if rustPkgs != null
        then rustPkgs.workspace.rust-symphony {}
        else null;
    in {
      packages = let
        rustSymphonyPkg = pkgs.rustPlatform.buildRustPackage {
          pname = "rust-symphony";
          version = "0.2.0";
          src = self;
          cargoLock.lockFile = self + "/Cargo.lock";
          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs = [pkgs.openssl];
          cargoBuildFlags = ["--package" "symphony-runner" "--bin" "symphony"];
          installPhase = ''
            runHook preInstall
            # Build the CLI binary from symphony-runner (bin name: symphony)
            cargo build --release --package symphony-runner --bin symphony
            mkdir -p $out/bin
            cp target/release/symphony $out/bin/rust-symphony
            runHook postInstall
          '';
        };
      in
        {
          "rust-symphony" = rustSymphonyPkg;
          default = rustSymphonyPkg;
          # buildRustPackage (always available)
          buildRustPackage = rustSymphonyPkg;
        }
        // pkgs.lib.optionalAttrs (cargo2nixPackage != null) {
          # cargo2nix workspace package (requires Cargo.nix in repo)
          cargo2nix = cargo2nixPackage;
        };

      apps = {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/rust-symphony";
        };
        # Generate Cargo.nix (run once, then commit Cargo.nix)
        generate = {
          type = "app";
          program = toString (pkgs.writers.writeBash "generate-cargo-nix" ''
            set -e
            nix run github:cargo2nix/cargo2nix/release-0.12 -- cargo2nix
            echo "Generated Cargo.nix - review and commit it."
          '');
        };
      };
    });
}
