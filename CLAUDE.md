# askic — Stage 2: Aski Compiler

Takes Stage 1's data-tree. Parses .aski source bodies using the
dialect-driven parser. Produces a fully-typed parse tree.

## Current State

The compiler is not yet built. The Rust bootstrap is being developed
across the three stage repos (synthc → askic → semac).

v0.15 reference code is in `v015_reference/` — 14 .aski modules
showing the previous compiler structure, useful as a guide.

## Repos

- **synthc** — Stage 1: .synth + .aski → data-tree + derived enums
- **askic** — Stage 2: data-tree + .aski bodies → typed parse tree
- **semac** — Stage 3: parse tree → .sema binary + codegen
- **aski** — language spec (`spec/pipeline.md`)

## VCS

Jujutsu (`jj`) mandatory. Always pass `-m`.
