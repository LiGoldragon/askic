# askic — The Aski Compiler

Reads .aski source, produces per-module .rkyv (ModuleDef).
Generic dialect engine — no language-specific parsing logic.
The dialect data from askicc IS the state machine.


## Engine Rewrite Design

The current engine has ParseValue::Seq (untyped bags), a
separate Builder that guesses nesting depth, as_* methods
that panic, and hard-coded alt_idx matching. All of this
goes away.

### Typed enum, no Seq

Every matched value carries its type. `Typed::Expr(Expr)`,
`Typed::PascalName(String, Span)`, `Typed::Exprs(Vec<Expr>)`.
No untyped bags. `into_*` methods return Result, never panic.

### ItemTuple for delimited groups

Delimited items like `(@Enum <Enum>)` produce an ItemTuple
(fixed-arity positional). Always immediately destructured
by the dialect method via `tuple.take(idx)`. Never stored.

### No separate builder

Each DialectKind has a `parse_*` method that reads the
dialect tree and constructs the output type directly.
`parse_root` builds ModuleDef. `parse_enum` builds
Vec<EnumChild>. `parse_type` builds TypeExpr.

### LabelKind classification, not alt_idx

The first LabelKind in an alternative's items tells the
engine what construct it matched. Enum, Struct, Trait,
Const, Newtype — from the dialect data, not from position.
Reordering synth alternatives doesn't break the engine.

### Cardinality-driven looping

Only alternatives with `*` or `+` cardinality loop.
Leaf dialects (Type, Pattern, Param) match once and stop.

### Typed collections from repeat

`*<Expr>` → `Typed::Exprs(Vec<Expr>)`. The inner item's
DialectKind determines the collection type. No Seq wrapping.


## Dependencies

synth-core (grammar types), aski-core (parse tree types),
rkyv, logos. Dialect data embedded via DIALECT_DATA env var.
