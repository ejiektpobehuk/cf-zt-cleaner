{
  description = "cf-zt-cleaner — reset CloudFlare Zero Trust users to a given permanent list";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;
        commonArgs = {
          inherit src;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        cf-zt-cleaner = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          meta = {
            description = "Reset CloudFlare Zero Trust users to a given permanent list";
            mainProgram = "cf-zt-cleaner";
          };
        });
      in
      {
        checks = {
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "-- -W clippy::pedantic -W clippy::nursery -W clippy::unwrap_used";
          });
          test = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        packages = {
          default = cf-zt-cleaner;

          docker = pkgs.dockerTools.buildLayeredImage {
            name = "cf-zt-cleaner";
            tag = "latest";
            # busybox provides /bin/sh + coreutils so the image is usable as a
            # GitLab CI job image (which spawns a shell for `script:`).
            contents = [ cf-zt-cleaner pkgs.busybox ];
            config = {
              Entrypoint = [ "${cf-zt-cleaner}/bin/cf-zt-cleaner" ];
              Cmd = [ "clean" "--auto-confirm" ];
              Env = [ "PATH=/bin:/usr/bin" ];
            };
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          packages = [
            pkgs.cargo-audit
            pkgs.cargo-msrv
            pkgs.clippy
            pkgs.rust-analyzer
          ];
        };
      }
    );
}
