# askic — Aski Compiler

The compiler stage of the sema engine. Takes askicc's binary output
(the data-tree) and parses .aski programs: expressions, statements,
match arms, scope enforcement. Produces a fully-typed parse tree.

Depends on askicc's **output** (the data-tree artifact) and the
type definitions from aski-core.

## Current State

Not yet built. Stubs in src/.

v0.15 reference code in `v015_reference/` — 14 .aski modules
showing the previous compiler structure.

## The Sema Engine

```
aski-core  →  askicc  →  askic  →  semac
(anatomy)    (bootstrap)  (compiler)  (sema gen)
```

## VCS

Jujutsu (`jj`) mandatory. Always pass `-m`.
