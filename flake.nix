{
  description = "askic — aski frontend: dialect state machine → rkyv parse tree";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    aski-core = {
      url = "github:LiGoldragon/aski-core";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
      inputs.flake-utils.follows = "flake-utils";
    };
    aski = {
      url = "github:Criome/aski";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
      inputs.flake-utils.follows = "flake-utils";
    };
    askicc = {
      url = "github:LiGoldragon/askicc";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
      inputs.flake-utils.follows = "flake-utils";
      inputs.aski-core.follows = "aski-core";
    };
  };

  outputs = { self, nixpkgs, fenix, crane, flake-utils,
              aski-core, aski, askicc, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.stable.toolchain;
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        # rkyv contract types
        aski-core-source = aski-core.packages.${system}.source;
        aski-source = aski.packages.${system}.source;

        # askicc's rkyv dialect-data-tree — embedded via include_bytes!
        dialect-data = askicc.packages.${system}.dialect-data;

        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*\\.aski$" path != null);
        };

        commonArgs = {
          inherit src;
          pname = "askic";
          version = "0.17.0";
          # Populate flake-crates/ for Cargo path deps
          postUnpack = ''
            mkdir -p $sourceRoot/flake-crates
            cp -r ${aski-core-source} $sourceRoot/flake-crates/aski-core
            cp -r ${aski-source} $sourceRoot/flake-crates/aski
            chmod -R +w $sourceRoot/flake-crates
          '';
          # askicc's rkyv output — embedded at build time
          DIALECT_DATA = "${dialect-data}/dialects.rkyv";
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        askic = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

      in {
        packages = {
          default = askic;
          inherit askic;
        };

        checks = {
          build = askic;
          tests = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        devShells.default = craneLib.devShell {
          packages = [ pkgs.rust-analyzer ];
          DIALECT_DATA = "${dialect-data}/dialects.rkyv";
        };
      }
    );
}
