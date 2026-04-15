# askic — Aski Compiler

Takes askicc's data-tree. Parses .aski source bodies using the
dialect-driven parser. Produces a fully-typed parse tree.

askic depends on askicc as a crate — reuses the core declaration
parser, lexer, and token reader. Adds body-level parsing on top:
expressions, statements, match arms, scope enforcement.

## Current State

The compiler is not yet built. The Rust bootstrap is being developed
across the compiler repos (askicc → askic → semac).

v0.15 reference code is in `v015_reference/` — 14 .aski modules
showing the previous compiler structure, useful as a guide.

## Repos

- **askicc** — bootstrap: .synth grammar + askic's .aski anatomy → data-tree
- **askic** — compiler: data-tree + .aski bodies → typed parse tree
- **semac** — sema generator: parse tree → .sema binary + codegen
- **aski** — language spec (`spec/pipeline.md`)

## VCS

Jujutsu (`jj`) mandatory. Always pass `-m`.
