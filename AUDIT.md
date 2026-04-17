# askic Engine Audit

## Architecture

```
.aski source → Lexer → tokens
                          ↓
ArchivedDialectTree → Engine → ParseValues
                                    ↓
                              Builder → sema-core types
                                    ↓
                              rkyv serialization → .rkyv output
```

Three layers:
- **Lexer** (logos) — tokenizes .aski source. 13 tests. Trusted.
- **Engine** (engine.rs) — generic dialect state machine. Walks
  ArchivedDialectTree, matches tokens, produces ParseValues.
  Should have NO sema-core knowledge.
- **Builder** (builder.rs) — per-dialect constructors. Converts
  ParseValues to sema-core types. ALL sema-core knowledge here.

Bridge type: **ParseValue** (values.rs) — intermediate data
between engine and builder.

## Current State

29 tests pass (13 lexer + 16 engine). nix flake check passes.
Parses v017 spec → 9 root children, 2392 bytes rkyv.

### What works:
- Module, Enum (all 5 variants), Struct (all 4 variants)
- Type expressions (Named, Application, Param, BoundedParam)
- Newtype, Const (int/float)
- Trait declarations
- Generic types ($Value, $Output, $Failure)
- Nested enums and structs
- Adjacency checking

### What's incomplete:
- Expression parsing untested
- Statement parsing untested
- Trait implementations untested
- Match/Pattern untested
- Loop/Iteration binding always Wildcard
- StringLit pattern always empty
- FFI untested
- Error cases untested

## Issues Found

### Critical (blocks correctness)

**1. Iteration binding always Wildcard**
builder.rs lines 760, 827 — `binding: Pattern::Wildcard`
The `{| source [body] |}` iteration ignores the binding.
In aski: `{| @Self.List.Item [@Item.Name] |}` — the `Item`
binding is lost. Blocks a core language feature.

**2. StringLit pattern empty**
builder.rs line 1230 — `Pattern::StringLitPattern(String::new())`
Match patterns like `("(")` produce empty strings.

**3. Float parsing silently defaults to 0.0**
engine.rs line 336 — `s.parse().unwrap_or(0.0)`
Invalid float tokens become 0.0 instead of errors.

### Architecture violations

**4. Engine constructs sema-core types**
engine.rs line 331-347 — `match_literal_value` creates
`sema_core::LiteralValue` directly. The engine should return
raw token data; the builder should construct sema-core types.
Violates the claimed "engine has NO sema-core knowledge."

**5. ExprPostfix special-cased in engine**
engine.rs lines 349-411 — `parse_postfix` handles left
recursion by directly calling builder.build_postfix().
Documented in DESIGN.md but creates coupling.

### Robustness

**6. 9 unwrap() calls in builders**
builder.rs — panic on internal inconsistencies instead of
returning errors. If synth changes or engine produces wrong
alternatives, these panic instead of reporting.

**7. Seq unwrapping in as_type_expr()**
values.rs line 92 — `ParseValue::Seq(v) if v.len() == 1`
Special case to unwrap optional results. Unpredictable API.

### Style

**8. Heavy cloning in ParseValue accessors**
values.rs — every as_*() method clones. Should provide
reference versions for reading.

**9. span_from_slice is identical to span_from_values**
builder.rs line 1366 — duplicate function.

**10. Free functions span_from_values and span_from_slice**
builder.rs lines 1359-1367 — should be methods.

## Test Coverage

| Construct | Tested |
|-----------|--------|
| Module | ✓ |
| Enum (bare) | ✓ |
| Enum (data-carrying) | ✓ |
| Enum (nested) | ✓ |
| Struct (typed fields) | ✓ |
| Struct (self-typed) | ✓ |
| Newtype | ✓ |
| Const (int/float) | ✓ |
| Trait declaration | ✓ |
| Generic types | ✓ |
| Type application | ✓ |
| v017 spec (integration) | ✓ |
| Expressions | ✗ |
| Statements | ✗ |
| Body/blocks | ✗ |
| Postfix (field/method/try) | ✗ |
| Binary operators | ✗ |
| Trait implementations | ✗ |
| Method definitions | ✗ |
| Match/Pattern | ✗ |
| Loop/Iteration | ✗ |
| Instance/Mutation | ✗ |
| FFI | ✗ |
| Error cases | ✗ |
| Span accuracy | ✗ |

## Line Counts

| File | Lines |
|------|-------|
| engine.rs | 509 |
| builder.rs | 1367 |
| values.rs | 183 |
| engine_tests.rs | 231 |
| lexer.rs | 269 |
| lexer_tests.rs | 139 |
| main.rs | 73 |
| lib.rs | 13 |
| **Total** | **2784** |

## Next Steps (priority order)

1. Fix iteration binding (consult on syntax)
2. Fix float parsing (return error)
3. Move LiteralValue construction from engine to builder
4. Convert unwrap() calls to errors
5. Add expression/statement/body tests
6. Add trait impl tests
7. Remove duplicate span function
8. Add error case tests
