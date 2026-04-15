{
  description = "askic — Stage 2: aski body parser";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    synthc = {
      url = "github:LiGoldragon/synthc";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
  };

  outputs = { self, nixpkgs, fenix, crane, synthc, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      toolchain = fenix.packages.${system}.stable.toolchain;
      craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

      synthc-bin = synthc.packages.${system}.synthc;
      synth-dialect = synthc.packages.${system}.synth-dialect;

      src = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          (craneLib.filterCargoSources path type)
          || (builtins.match ".*\\.aski$" path != null);
      };

      commonArgs = {
        inherit src;
        pname = "askic";
        version = "0.16.0";
        nativeBuildInputs = [ synthc-bin ];
        SYNTH_DIR = "${synth-dialect}";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      askic = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
      });

    in {
      packages.${system} = {
        default = askic;
        inherit askic;
      };

      checks.${system} = {
        build = askic;
        cargo-tests = craneLib.cargoTest (commonArgs // {
          inherit cargoArtifacts;
        });
      };

      devShells.${system}.default = craneLib.devShell {
        packages = [ synthc-bin pkgs.rust-analyzer ];
        SYNTH_DIR = "${synth-dialect}";
      };
    };
}
