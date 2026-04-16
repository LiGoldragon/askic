# Second Pass Cleanup Notes

## CRITICAL: parse.rs is WRONG

parse.rs is a hand-written recursive descent parser that bypasses
askicc's dialect data entirely. It produces correct output but
through the wrong mechanism. It must be REPLACED with a generic
dialect engine that reads askicc's generated dialect structures
and drives parsing from them.

The domain types (domain.rs), lexer, rkyv serialization (sema.rs),
and tests are correct and should be kept. Only parse.rs needs
replacement.

## Things to revisit after the bootstrap works.

## askicc
- Synth lexer `<`/`>` handling: 4 match arms for 2 concepts. Unify.
- Synth lexer catch-all operator branch: exclusion set is fragile.
- Codegen reserved word table: should be generated, not hand-maintained.
- `domain_tree.rs` needs rkyv derives for .sema serialization.
- `dialects.rs` output is just comments — needs actual Rust dialect data.
- Synth parser inline-or (`//` inside delimiters) stored as Literal("//")
  — should be a proper InlineOr variant.

## askic
- rkyv recursive type overflow: derive macro can't handle TypeExpr/Expr/Statement cycles.
  Need manual Archive impl or structural changes (arena + indices instead of Box).
  Bootstrap uses debug-format placeholder serialization.
- Statement @Name ambiguity: @Name followed by . or operator must be parsed as
  expression, not instance declaration. Current fix: peek-ahead check. Fragile.
- No-arg method calls: .method without () — currently supported, but should this
  be in the spec? Or should all method calls require ()?
- Loop condition parsing: currently parses everything as statements, no condition
  detection. Needs heuristic or syntax change.
- Module export parsing reads camelCase as Name but module name is PascalCase.
  Module declaration parsing is fragile with mixed casings.
