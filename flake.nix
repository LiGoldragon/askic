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
    synth-core = {
      url = "github:LiGoldragon/synth-core";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
      inputs.flake-utils.follows = "flake-utils";
    };
    aski-core = {
      url = "github:Criome/aski-core";
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
      inputs.synth-core.follows = "synth-core";
    };
  };

  outputs = { self, nixpkgs, fenix, crane, flake-utils,
              synth-core, aski-core, askicc, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.stable.toolchain;
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        # rkyv contract types
        synth-core-source = synth-core.packages.${system}.source;
        aski-core-source = aski-core.packages.${system}.source;

        # askicc's rkyv dsls-data-tree — embedded via include_bytes!
        dsls-data = askicc.packages.${system}.dsls-data;

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
          postUnpack = ''
            mkdir -p $sourceRoot/flake-crates
            cp -r ${synth-core-source} $sourceRoot/flake-crates/synth-core
            cp -r ${aski-core-source} $sourceRoot/flake-crates/aski-core
            chmod -R +w $sourceRoot/flake-crates
          '';
          DIALECT_DATA = "${dsls-data}/dsls.rkyv";
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
          DIALECT_DATA = "${dsls-data}/dsls.rkyv";
        };
      }
    );
}
