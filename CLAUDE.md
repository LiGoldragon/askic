# askic — The Aski Compiler

Reads `.core` / `.aski` / `.synth` / `.exec` source files. Produces
per-file rkyv conforming to aski-core types. Generic dialect engine
— no language-specific parsing logic in source. The dialect data
from askicc IS the state machine.

**v0.19 status: wiped, awaiting rewrite.** All pre-v0.18 engine code
(builder.rs, machine.rs, engine.rs, typed.rs, values.rs, lexer.rs,
main.rs) was deleted on 2026-04-18. Only `lib.rs` (one-line
placeholder), `Cargo.toml` (v0.18.0), `flake.nix`, `LICENSE.md`,
and this `CLAUDE.md` remain. Rewrite blocks on **askic-assemble**.

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

## v0.19 syntax (what the engine must parse)

- `&self`, `~&self` for borrows (was `:@Self`, `~@Self`)
- `Type:method(args)`, `Type:Variant` for paths (was `/`)
- `{Vec Element}` type application (was `[]`)
- `[Fire Air]` or-patterns (was `(Fire | Air)`)
- `(counter U32:new(0))` local decls with camel names (was `@Counter U32/new(0)`)
- `[expr]` ExprStmt (was bare)
- `{$Value}` generic slot after definition name
- Pascal traits, camel locals/methods/self
- No `@` sigil anywhere in aski source

See `/home/li/git/aski-core/spec/syntax-v019.aski` for examples.

---

## Dependencies

- **synth-core** — grammar contract types (Dialect, Rule, Item,
  Tag, Label, TagKind, LabelKind, DialectKind, SurfaceKind, …)
- **aski-core** — parse-tree output types (Module, Enum, Struct,
  Method, Type, Param, Origin, Expr, Statement, Pattern, Body,
  LocalDecl, Loop, Program, …)
- **askic-assemble** — proc-macro that generates the per-tag
  dispatch code at compile time (does not exist yet — blocker)
- **rkyv**, **logos** (optional) — third-party

dsls.rkyv comes from askicc (via flake input or env var
`DSLS_RKYV`).

---

## Current code state (empty shell, 2026-04-19)

- `src/lib.rs` — one-line placeholder
- `Cargo.toml` — 0.18.0, no `[[bin]]` entry
- `flake.nix`, `LICENSE.md`, `CLAUDE.md` — shell

Nothing to cargo-cult on during the rewrite.

---

## VCS

`jj` mandatory. Git is storage backend only.
