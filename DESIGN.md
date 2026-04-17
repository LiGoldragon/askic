# askic Engine Design

## Constraints

**Input:** ArchivedDialectTree (aski-core types, rkyv zero-copy)
**Output:** Vec<RootChild> (sema-core types, rkyv serialized)

The engine has NO language knowledge. It follows the embedded
dialect data mechanically. All sema-core construction logic
lives in per-dialect builder methods.

## Architecture: Three Layers

```
Lexer (existing)     — .aski source → tokens
Engine               — tokens × dialect data → ParseValues
Builders (per-dialect) — ParseValues → sema-core types
```

### Layer 1: Lexer
Already exists. 13 tests pass. Produces Vec<Spanned> tokens.

### Layer 2: Engine (generic, no language knowledge)
Walks the ArchivedDialectTree. For each rule/item, matches
tokens and produces intermediate ParseValues. The engine
knows about:
- Named items → read a token of the right casing
- DialectRef → recurse into another dialect
- Delimited → match open, parse inner, match close
- Literal → match exact token
- Keyword → match exact keyword
- Repeat → loop with cardinality
- LiteralValue → read a literal token
- Adjacency → check token spans

The engine does NOT know about Expr, Statement, Block, etc.

### Layer 3: Builders (all sema-core knowledge)
One method per DialectKind. Receives ParseValues from the
engine, constructs sema-core types. The builder knows:
- Which alternative index maps to which sema-core variant
- Which ParseValue position is the name, type, expr, etc.
- Which name wrapper to use (TypeName, FieldName, etc.)
- How to compute spans

## ParseValue — the intermediate representation

```rust
enum ParseValue {
    Name(String, Span),           // from Named items
    Literal(LiteralValue, Span),  // from LiteralValue items
    Token(Span),                  // from Literal/Keyword (just confirms match)
    Seq(Vec<ParseValue>),         // from Delimited inner / Repeat
    Dialect(SemaValue),           // from DialectRef (recursive result)
}
```

SemaValue wraps all sema-core types the builders can produce:

```rust
enum SemaValue {
    RootChildren(Vec<RootChild>),
    EnumChildren(Vec<EnumChild>),
    StructChildren(Vec<StructChild>),
    Expr(Expr),
    Statement(Statement),
    Block(Block),
    TypeExpr(TypeExpr),
    TypeApplication(TypeApplication),
    Pattern(Pattern),
    MatchExpr(MatchExpr),
    LoopExpr(LoopExpr),
    Iteration(Iteration),
    Param(Param),
    MethodSig(MethodSig),
    MethodDef(MethodDef),
    MethodBody(MethodBody),
    ModuleDef(ModuleDef),
    Instance(Instance),
    Mutation(Mutation),
    FfiFunction(FfiFunction),
    // etc — one variant per sema-core type a dialect can return
}
```

## Engine Flow

1. `Engine::parse(source) -> Vec<RootChild>`
2. Engine lexes source → tokens
3. Engine enters Root dialect
4. For each rule in Root:
   - Sequential: match items left-to-right, collect ParseValues
   - OrderedChoice: try alternatives, first match wins
   - RepeatedChoice: loop trying alternatives until none match
5. After matching, call `build_root(matched_rules) -> SemaValue::RootChildren`
6. Root builder dispatches per-alternative:
   - alt 0: extract name + Enum children → RootChild::Enum(EnumDef)
   - alt 1: extract name + TraitDecl → RootChild::TraitDecl(...)
   - etc.
7. Serialize Vec<RootChild> as rkyv

## Builder Methods

One per DialectKind (minus pass-throughs):

| Dialect | Returns | Notes |
|---------|---------|-------|
| Root | RootChildren | seq(Module) + repeated choice |
| Module | ModuleDef | seq: exports + imports |
| Enum | EnumChildren | repeated choice, 5 alts |
| Struct | StructChildren | repeated choice, 4 alts |
| Body | Block | seq: statements + tail expr |
| ExprOr | Expr | choice: BinOr or fallthrough |
| ExprAnd | Expr | choice: BinAnd or fallthrough |
| ExprCompare | Expr | choice: 6 comparison ops or fallthrough |
| ExprAdd | Expr | choice: BinAdd/BinSub or fallthrough |
| ExprMul | Expr | choice: BinMul/BinMod or fallthrough |
| ExprPostfix | Expr | left-recursive: field/method/try or fallthrough |
| ExprAtom | Expr | choice: 10 leaf expression forms |
| Type | TypeExpr | choice: 4 type forms |
| TypeApplication | TypeApplication | seq: constructor + args |
| GenericParam | TypeExpr | choice: bounded or bare role |
| Statement | Statement | choice: 10 statement forms |
| Instance | Instance | choice: with/without type annotation |
| Mutation | Mutation | seq: instance.method(args) |
| Param | Param | choice: 7 param forms |
| Signature | MethodSig | seq: params + return type |
| Method | MethodDef | seq + choice for body |
| TraitDecl | TraitDeclDef | seq: signatures in brackets |
| TraitImpl | TraitImplDef | seq: type + methods |
| TypeImpl | Vec<MethodDef> | seq: repeated methods |
| Match | MatchExpr | seq: optional target + arms |
| Pattern | Pattern | choice: 3 pattern forms |
| Loop | LoopExpr | choice: conditional or infinite |
| Process | Block | seq: statements |
| StructConstruct | (TypeName, Vec<FieldInit>) | seq: struct + fields |
| Ffi | FfiDef | seq: repeated functions |

## Pass-Through Handling

Expr.synth → delegates to ExprOr. No builder needed.
Expression fallthroughs (last alt in each Expr* dialect) →
return the sub-dialect result directly, no wrapping.

## ExprPostfix — Left Recursion

ExprPostfix is left-recursive:
```
// <ExprPostfix>.:Field
// <ExprPostfix>.:method(+<Expr>)
// <ExprPostfix>_?_
// <ExprAtom>
```

The engine handles this iteratively:
1. Parse <ExprAtom> first (the base case, alt 3)
2. Loop: try alts 0-2 with the accumulated result as "left"
3. Each match wraps the result: FieldAccess(prev, field)
4. When no alt matches, return accumulated result

## File Structure

```
src/
  main.rs          — CLI: load dialect data, lex, parse, serialize
  lib.rs           — module declarations
  lexer.rs         — existing lexer (13 tests)
  lexer_tests.rs   — existing tests
  engine.rs        — generic dialect engine (matching)
  engine_tests.rs  — engine tests
  value.rs         — ParseValue + SemaValue enums
  builder.rs       — per-dialect builder methods
  builder_tests.rs — builder tests
```

## Estimated Size

| File | Lines |
|------|-------|
| engine.rs | ~300 |
| value.rs | ~100 |
| builder.rs | ~800 |
| main.rs | ~50 |
| tests | ~400 |
| **Total** | ~1650 |
