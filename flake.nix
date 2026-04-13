{
  description = "askic — the aski compiler, written in aski";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    aski-core-src = {
      url = "github:LiGoldragon/aski-core";
      flake = false;
    };
    aski-rs-bootstrap-src = {
      url = "github:LiGoldragon/aski-rs/askic-bootstrap";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, fenix, crane, aski-core-src, aski-rs-bootstrap-src, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      toolchain = fenix.packages.${system}.stable.toolchain;
      craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

      # Build stage1 (bootstrap) from source
      bootstrapSrc = pkgs.lib.cleanSourceWith {
        src = aski-rs-bootstrap-src;
        filter = path: type:
          (craneLib.filterCargoSources path type) ||
          (builtins.match ".*\\.aski$" path != null) ||
          (builtins.match ".*\\.synth$" path != null);
      };
      bootstrapArgs = { pname = "askic-bootstrap"; version = "0.15.0"; src = bootstrapSrc; };
      bootstrapDeps = craneLib.buildDepsOnly bootstrapArgs;
      stage1 = craneLib.buildPackage (bootstrapArgs // { cargoArtifacts = bootstrapDeps; });

      synth-dir = "${aski-core-src}/source";

      # Compile each .aski module with stage1 and verify with rustc
      stage2-type-check = pkgs.runCommand "askic-stage2-type-check" {
        nativeBuildInputs = [ stage1 toolchain ];
      } ''
        set -euo pipefail
        mkdir -p $out

        echo "=== Stage2 module compilation ==="
        for name in sema synth world; do
          work=$(mktemp -d)
          cp ${./source}/$name.aski $work/
          askic rust $work/$name.aski --synth-dir ${synth-dir} > $out/$name.rs
          rustc $out/$name.rs --crate-type lib -o $out/lib$name.rlib
          echo "  ✓ $name.aski → .rs compiles ($(wc -l < $out/$name.rs) lines)"
        done

        echo ""
        echo "=== Stage2 sema artifact check ==="
        for name in sema synth world; do
          work=$(mktemp -d)
          cp ${./source}/$name.aski $work/
          askic compile $work/$name.aski --synth-dir ${synth-dir}
          sema="$work/$name.sema"
          table="$work/$name.aski-table.sema"
          test -f "$sema" || (echo "FAIL: $sema not found"; exit 1)
          test -f "$table" || (echo "FAIL: $table not found"; exit 1)
          echo "  ✓ $name.sema ($(stat -c%s "$sema") bytes)"
        done

        echo ""
        echo "=== All stage2 checks passed ==="
      '';

    in {
      packages.${system} = {
        bootstrap = stage1;
      };

      checks.${system} = {
        stage2-types = stage2-type-check;
      };

      devShells.${system}.default = craneLib.devShell {
        packages = [ stage1 ];
      };
    };
}
