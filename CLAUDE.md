# askic — The Aski Compiler

Reads `.core` / `.aski` / `.synth` / `.exec` source files. Produces
per-file rkyv conforming to aski-core types. Generic dialect engine
— no language-specific parsing logic in source. The dialect data
from askicc IS the state machine.

**v0.18 status: pending full rewrite.** Current machine.rs / engine.rs
/ builder.rs are pre-v0.18 and reference the old aski-core types
(ModuleDef, etc.) that were redesigned. Needs ground-up rewrite
against the new contract.

---

## Engine design (target)

### Pure dsl-driven state machine
askic engine source has **zero hardcoded grammar terms**. No
`TagKind::Enum` variant names in the engine. The engine is
infrastructure: reads source, walks dsls.rkyv, tracks adjacency,
handles cardinality, dispatches on tags.

### Per-tag construction via proc-macro
The engine hands matched items to `askic_assemble::assemble_from_tag(tag,
items)` — a function generated at compile time by the
`askic-assemble` proc-macro crate. That crate reads dsls.rkyv +
aski-core's .core types and emits one match arm per TagKind, each
populating the corresponding `aski_core::` entity.

Result: engine source has ONE macro invocation
(`askic_assemble::assemble_from_dsls!(env!("DSLS_RKYV"))`) and
the grammar-term dispatch code exists in the expanded output,
computed from dsls.rkyv — not hand-written.

### Single combined rkyv
askicc emits ONE `dsls.rkyv` containing all four DSLs' dialects,
each tagged with its `SurfaceKind`. askic loads the one file
at compile time (via `include_bytes!`). Dispatch is a flat
`HashMap<(SurfaceKind, DialectKind), idx>`. File extension
(`.core` / `.aski` / `.synth` / `.exec`) picks which surface's
`Root` dialect to enter.

### Tag dispatch, not alt_idx, not LabelKind
Each synth alternative has a `#Tag#` (TagKind). The engine uses
TagKind as the dispatch key. `LabelKind` is the ROLE of a
source-read identifier within a construct; it's not the engine's
dispatch key. Keep the two enums distinct (they are, in synth-core).

---

## Dependencies

- **synth-core** — grammar contract types (Dialect, Rule, Item,
  Tag, Label, TagKind, LabelKind, DialectKind, SurfaceKind, …)
- **aski-core** — parse-tree output types (Module, Enum, Struct,
  Method, Type, Param, Origin, Expr, Statement, Pattern, Body, …)
- **askic-assemble** — proc-macro that generates the per-tag
  dispatch code at compile time
- **rkyv**, **logos** — third-party

dsls.rkyv comes from askicc (via flake input or env var
`DSLS_RKYV`).

---

## Current code state (pre-v0.18, awaiting rewrite)

- `builder.rs` — OLD per-dialect builder methods. **Delete entirely**
  on rewrite.
- `machine.rs` — has partial typed-value infrastructure (`Typed`
  enum, `ItemTuple`) but still has per-dialect parse methods.
  **Full rewrite.**
- `engine.rs` — old lexer/dispatcher. **Full rewrite** to generic
  TagKind-dispatched state machine.
- `typed.rs` — `Typed` enum may survive as intermediate type
  between engine and `assemble_from_tag`.
- `values.rs` — old value types. Likely deleted.

**7 ignored tests** — all test builder.rs bugs. Will be deleted
with builder.rs. The test suite gets rewritten against the new
engine's observable behavior.

---

## VCS

`jj` mandatory. Git is storage backend only.
