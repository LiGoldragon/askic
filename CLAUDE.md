# askic — The Aski Compiler

Reads `.core`/`.aski`/`.synth`/`.exec` source files and produces
per-file rkyv. Generic dialect engine — no language-specific
parsing logic. The dialect data from askicc IS the state machine.


## v0.18 — Surface Dispatch

askic dispatches on file extension:

- `.core` → loads `dialects.core.rkyv`, outputs core types
- `.aski` → loads `dialects.aski.rkyv`, outputs ModuleDef
- `.synth` → loads `dialects.synth.rkyv` (tooling use only)
- `.exec` → loads `dialects.exec.rkyv`, outputs ExecProgram

Each surface's dialect tree is loaded independently from
`<DIALECT_ROOT>/dialects.<surface>.rkyv`. Cross-surface
dialect refs (`<:surface:Name>` in the source `.synth`) are
resolved by loading the referenced surface's tree too.


## Engine Design (pending full implementation)

The engine is fully data-driven. The dialect data tells it
everything — no per-dialect hard-coded methods, no alt_idx,
no guessing.

**Current state:** partial typed-value infrastructure
(`typed.rs` has `Typed` enum and `ItemTuple`). `machine.rs`
has some per-dialect methods as scaffolding. Old engine and
builder still present.

**Target state:** one generic `assemble_from_label(LabelKind,
...)` function that dispatches construction. No `parse_root`,
no `parse_enum`, no `parse_type`, etc. The dialect data's
`@Label` and `#Tag#` identify every output variant.


## Design Principles

### Typed values, no Seq

Every matched value carries its type. `Typed::Expr(Expr)`,
`Typed::PascalName(String, Span)`, `Typed::Exprs(Vec<Expr>)`.
No untyped bags. `into_*` methods return Result, never panic.

### ItemTuple for delimited groups

Delimited items produce a fixed-arity positional tuple,
immediately destructured via `tuple.take(idx)`. Never stored.

### LabelKind dispatch, not alt_idx

The `@Label` or `#Tag#` in each alternative tells the engine
what construct it matched. Dispatching on alt_idx is fragile
(reorder a synth alternative → break). Dispatching on
LabelKind is robust — the synth grammar is the source of truth.

### Cardinality-driven looping

Only alternatives with `*` or `+` cardinality loop.
Leaf dialects (Type, Pattern, Param) match once and stop.

### Typed collections from repeat

`*<Expr>` → `Typed::Exprs(Vec<Expr>)`. The inner item's
DialectKind determines the collection type. No Seq wrapping.


## Dependencies

synth-core (grammar types), aski-core (parse tree types),
rkyv, logos. Dialect data from askicc.
