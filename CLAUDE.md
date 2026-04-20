# askic — The Aski Compiler

Reads `.core` / `.aski` / `.synth` / `.exec` source files. Produces
per-file rkyv conforming to aski-core types. Generic dialect engine
— no language-specific parsing logic in source. The dialect data
from askicc IS the state machine.

**v0.20 status: wiped, awaiting rewrite.** All pre-v0.18 engine code
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
askicc emits ONE `dsls.rkyv` containing all five DSLs' dialects,
each tagged with its `SurfaceKind`. askic loads the one file
at compile time (via `include_bytes!`). Dispatch is a flat
`HashMap<(SurfaceKind, DialectKind), idx>`. File extension
(`.core` / `.aski` / `.synth` / `.exec` / `.rfi`) picks which
surface's `Root` dialect to enter.

### Tag dispatch, not alt_idx, not LabelKind
Each synth alternative has a `#Tag#` (TagKind). The engine uses
TagKind as the dispatch key. `LabelKind` is the ROLE of a
source-read identifier within a construct; it's not the engine's
dispatch key. Keep the two enums distinct (they are, in synth-core).

---

## v0.20 syntax (what the engine must parse)

In addition to v0.19's sigil/delimiter changes (`&` borrow, `:` path,
`~&` mutable borrow, `{}` type app, `[]` or-pattern, camel locals,
no `@` for instance), v0.20 adds:

- **`@` sigil** for VISIBILITY (public) on declarations and fields
- **Trait delimiter** moved from `(...)` to `[|...|]` at root (every root
  construct now has a distinct opening token — first-token decidable)
- **Associated types** in traits: bare Pascal name (decl) or
  `(Pascal Type)` (impl binding). New `SelfAssocType` for `self:Item`
  paths in signatures.
- **`self` as expression atom** — `self.field`, `self.method()` now
  parse. New `SelfRef` ExprAtom variant.
- **RFI moved to own surface** (`.rfi` files, `SurfaceKind::Rfi`).
- **Module.Exports retired** — visibility is declaration-local.

### Previous v0.19 mechanics:

- `&self`, `~&self` for borrows (was `:@Self`, `~@Self`)
- `Type:method(args)`, `Type:Variant` for paths (was `/`)
- `{Vec Element}` type application (was `[]`)
- `[Fire Air]` or-patterns (was `(Fire | Air)`)
- `(counter U32:new(0))` local decls with camel names (was `@Counter U32/new(0)`)
- `[expr]` ExprStmt (was bare)
- `{$Value}` generic slot after definition name
- Pascal traits, camel locals/methods/self
- No `@` sigil anywhere in aski source

See `/home/li/git/aski/spec/syntax-v020.aski` for examples.

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
