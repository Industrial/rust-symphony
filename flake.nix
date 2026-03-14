{
  description = "Symphony orchestrator and Firecracker sandbox (kernel, guest rootfs, runner)";
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

      rustSymphonyPkg = pkgs.rustPlatform.buildRustPackage {
        pname = "rust-symphony";
        version = "0.3.1";
        src = self;
        cargoLock.lockFile = self + "/Cargo.lock";
        nativeBuildInputs = [pkgs.pkg-config];
        buildInputs = [pkgs.openssl];
        cargoBuildFlags = ["--package" "symphony-runner" "--bin" "symphony"];
        installPhase = ''
          runHook preInstall
          cargo build --release --package symphony-runner --bin symphony
          mkdir -p $out/bin
          cp target/release/symphony $out/bin/rust-symphony
          runHook postInstall
        '';
      };

      symphonyGuestRunnerPkg = pkgs.rustPlatform.buildRustPackage {
        pname = "symphony-guest-runner";
        version = "0.3.1";
        src = self;
        cargoLock.lockFile = self + "/Cargo.lock";
        cargoBuildFlags = ["--package" "symphony-guest-runner" "--bin" "symphony-guest-runner"];
        doCheck = false;
        installPhase = ''
          runHook preInstall
          cargo build --release --package symphony-guest-runner --bin symphony-guest-runner
          mkdir -p $out/bin
          cp target/release/symphony-guest-runner $out/bin/symphony-guest-runner
          runHook postInstall
        '';
      };
    in {
      packages =
        {
          "rust-symphony" = rustSymphonyPkg;
          default = rustSymphonyPkg;
          buildRustPackage = rustSymphonyPkg;
          "symphony-guest-runner" = symphonyGuestRunnerPkg;
        }
        // pkgs.lib.optionalAttrs (cargo2nixPackage != null) {
          cargo2nix = cargo2nixPackage;
        }
        # Firecracker kernel and guest rootfs (x86_64-linux only).
        # Default kernel is used for cache; for vsock guest–host communication a kernel with
        # CONFIG_VIRTIO_VSOCKETS may be required (e.g. custom override or use linuxPackages from a profile that has it).
        // pkgs.lib.optionalAttrs (system == "x86_64-linux") (let
          vmlinuxFirecracker = pkgs.linuxPackages.kernel.dev;
          rootfsTree = pkgs.runCommand "symphony-guest-rootfs-tree" {} ''
            mkdir -p rootfs/bin rootfs/dev rootfs/proc rootfs/sys rootfs/worktree
            cp ${symphonyGuestRunnerPkg}/bin/symphony-guest-runner rootfs/bin/
            echo '#!/bin/sh' > rootfs/init
            echo 'mount -t proc none /proc' >> rootfs/init
            echo 'mount -t sysfs none /sys' >> rootfs/init
            echo 'exec /bin/symphony-guest-runner' >> rootfs/init
            chmod +x rootfs/init
            ln -s ${pkgs.busybox}/bin/busybox rootfs/bin/sh
            ln -s ${pkgs.busybox}/bin/busybox rootfs/bin/mount
            touch rootfs/dev/null
            cp -r rootfs $out
          '';
          rootfsExt4 =
            pkgs.runCommand "symphony-guest-rootfs.ext4" {
              nativeBuildInputs = [pkgs.genext2fs pkgs.e2fsprogs];
            } ''
              genext2fs -d ${rootfsTree} -b 65536 -N 0 $out
              e2fsck -y -f $out || true
              resize2fs -M $out
            '';
        in {
          firecracker = pkgs.firecracker;
          vmlinuxFirecracker = vmlinuxFirecracker;
          symphony-guest-rootfs-tree = rootfsTree;
          symphony-guest-rootfs = rootfsExt4;
        });

      apps = {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/rust-symphony";
        };
        generate = {
          type = "app";
          program = toString (pkgs.writers.writeBash "generate-cargo-nix" ''
            set -e
            nix run github:cargo2nix/cargo2nix/release-0.12 -- cargo2nix
            echo "Generated Cargo.nix - review and commit it."
          '');
        };
        build-guest-rootfs = pkgs.lib.optionalAttrs (system == "x86_64-linux") {
          type = "app";
          program = toString (pkgs.writers.writeBash "build-guest-rootfs" ''
            set -e
            out="''${OUT:-./symphony-guest-rootfs.ext4}"
            echo "Building guest rootfs (tree + ext4)..."
            nix build --no-link .#"symphony-guest-rootfs"
            cp $(nix path-info .#"symphony-guest-rootfs") "$out"
            echo "Wrote $out"
          '');
        };
      };
    });
}
