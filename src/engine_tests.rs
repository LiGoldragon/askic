#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::engine::Engine;
    use aski_core::*;

    fn load_dialect_data() -> &'static [u8] {
        // Try DIALECT_DATA env var first (set by nix build)
        if let Ok(path) = std::env::var("DIALECT_DATA") {
            let bytes = std::fs::read(&path).expect("failed to read DIALECT_DATA");
            return Box::leak(bytes.into_boxed_slice());
        }
        // Local dev fallback
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../askicc/generated/dialects.rkyv");
        let bytes = std::fs::read(&path).expect("dialect data not found — run askicc first");
        Box::leak(bytes.into_boxed_slice())
    }

    fn parse(source: &str) -> ModuleDef {
        let tokens = lex(source).expect("lex failed");
        let engine = Engine::new(load_dialect_data());
        engine.parse(&tokens).expect("parse failed")
    }

    // ── Module ──────────────────────────────────────────────

    #[test]
    fn parse_module_only() {
        let module = parse("(Elements Element Quality)");
        assert_eq!(module.name.0, "Elements");
        assert_eq!(module.exports.len(), 2);
    }

    // ── Enum ────────────────────────────────────────────────

    #[test]
    fn parse_bare_enum() {
        let module = parse("(T E)\n(Element Fire Earth Air Water)");
        assert_eq!(module.enums.len(), 1);
        let e = &module.enums[0];
        assert_eq!(e.name.0, "Element");
        assert_eq!(e.children.len(), 4);
        match &e.children[0] {
            EnumChild::Variant { name, .. } => assert_eq!(name.0, "Fire"),
            _ => panic!("expected Variant"),
        }
    }

    #[test]
    fn parse_data_carrying_enum() {
        let module = parse("(T E)\n(Option (Some String) None)");
        assert_eq!(module.enums.len(), 1);
        let e = &module.enums[0];
        assert_eq!(e.name.0, "Option");
        assert_eq!(e.children.len(), 2);
        match &e.children[0] {
            EnumChild::DataVariant { name, payload, .. } => {
                assert_eq!(name.0, "Some");
                assert!(matches!(payload, TypeExpr::Named(t) if t.0 == "String"));
            }
            _ => panic!("expected DataVariant"),
        }
        match &e.children[1] {
            EnumChild::Variant { name, .. } => assert_eq!(name.0, "None"),
            _ => panic!("expected Variant"),
        }
    }

    #[test]
    fn parse_type_application_in_enum() {
        let module = parse("(T E)\n(ParseResult (Success [Vec Node]) (Failure String))");
        let e = &module.enums[0];
        match &e.children[0] {
            EnumChild::DataVariant { payload, .. } => {
                match payload {
                    TypeExpr::Application(ta) => {
                        assert_eq!(ta.constructor.0, "Vec");
                        assert_eq!(ta.args.len(), 1);
                    }
                    _ => panic!("expected Application"),
                }
            }
            _ => panic!("expected DataVariant"),
        }
    }

    #[test]
    fn parse_nested_enum() {
        let module = parse(
            "(T E)\n(Token (Ident String) (Number I64) (| Delimiter LParen RParen |) Newline)"
        );
        let e = &module.enums[0];
        assert_eq!(e.name.0, "Token");
        assert!(e.children.len() >= 4);
        match &e.children[2] {
            EnumChild::NestedEnum(nested) => {
                assert_eq!(nested.name.0, "Delimiter");
                assert_eq!(nested.children.len(), 2);
            }
            _ => panic!("expected NestedEnum, got {:?}", e.children[2]),
        }
    }

    // ── Struct ──────────────────────────────────────────────

    #[test]
    fn parse_struct() {
        let module = parse("(T E)\n{Point (Horizontal F64) (Vertical F64)}");
        assert_eq!(module.structs.len(), 1);
        let s = &module.structs[0];
        assert_eq!(s.name.0, "Point");
        assert_eq!(s.children.len(), 2);
        match &s.children[0] {
            StructChild::TypedField { name, typ, .. } => {
                assert_eq!(name.0, "Horizontal");
                assert!(matches!(typ, TypeExpr::Named(t) if t.0 == "F64"));
            }
            _ => panic!("expected TypedField"),
        }
    }

    #[test]
    fn parse_self_typed_field() {
        let module = parse("(T E)\n{Drawing (Shapes [Vec Shape]) Name}");
        let s = &module.structs[0];
        assert_eq!(s.children.len(), 2);
        match &s.children[1] {
            StructChild::SelfTypedField { name, .. } => assert_eq!(name.0, "Name"),
            _ => panic!("expected SelfTypedField"),
        }
    }

    // ── Newtype ─────────────────────────────────────────────

    #[test]
    fn parse_newtype() {
        let module = parse("(T E)\nCounter U32");
        assert_eq!(module.newtypes.len(), 1);
        let n = &module.newtypes[0];
        assert_eq!(n.name.0, "Counter");
        assert!(matches!(&n.wraps, TypeExpr::Named(t) if t.0 == "U32"));
    }

    #[test]
    fn parse_newtype_with_type_application() {
        let module = parse("(T E)\nItems [Vec Item]");
        let n = &module.newtypes[0];
        assert_eq!(n.name.0, "Items");
        match &n.wraps {
            TypeExpr::Application(ta) => {
                assert_eq!(ta.constructor.0, "Vec");
            }
            _ => panic!("expected Application"),
        }
    }

    // ── Const ───────────────────────────────────────────────

    #[test]
    fn parse_const_int() {
        let module = parse("(T E)\n{| MaxSigns U32 12 |}");
        assert_eq!(module.consts.len(), 1);
        let c = &module.consts[0];
        assert_eq!(c.name.0, "MaxSigns");
        assert!(matches!(&c.typ, TypeExpr::Named(t) if t.0 == "U32"));
        assert!(matches!(&c.value, LiteralValue::Int(12)));
    }

    #[test]
    fn parse_const_float() {
        let module = parse("(T E)\n{| Pi F64 3.14 |}");
        let c = &module.consts[0];
        assert_eq!(c.name.0, "Pi");
        match &c.value {
            LiteralValue::Float(v) => assert!((v - 3.14).abs() < 0.001),
            _ => panic!("expected Float"),
        }
    }

    // ── Trait Declaration ───────────────────────────────────

    #[test]
    fn parse_trait_decl() {
        let module = parse("(T E describe)\n(describe [(describe :@Self Quality)])");
        assert_eq!(module.trait_decls.len(), 1);
        let td = &module.trait_decls[0];
        assert_eq!(td.name.0, "describe");
        assert_eq!(td.signatures.len(), 1);
        assert_eq!(td.signatures[0].name.0, "describe");
    }

    // ── Multiple constructs ─────────────────────────────────

    #[test]
    fn parse_multiple_constructs() {
        let source = "\
(Elements Element Quality)

(Element Fire Earth Air Water)
(Quality Passionate Grounded Intellectual Intuitive)

{Point (Horizontal F64) (Vertical F64)}

Counter U32

{| MaxSigns U32 12 |}";
        let module = parse(source);
        assert_eq!(module.name.0, "Elements");
        assert_eq!(module.enums.len(), 2);
        assert_eq!(module.structs.len(), 1);
        assert_eq!(module.newtypes.len(), 1);
        assert_eq!(module.consts.len(), 1);
    }

    // ── Generic types ───────────────────────────────────────

    #[test]
    fn parse_generic_enum() {
        let module = parse("(T E)\n(Option (Some $Value) None)");
        let e = &module.enums[0];
        match &e.children[0] {
            EnumChild::DataVariant { payload, .. } => {
                assert!(matches!(payload, TypeExpr::Param(p) if p.0 == "Value"));
            }
            _ => panic!("expected DataVariant"),
        }
    }

    #[test]
    fn parse_two_param_generic() {
        let module = parse("(T E)\n(Result (Ok $Output) (Err $Failure))");
        let e = &module.enums[0];
        assert_eq!(e.children.len(), 2);
        match &e.children[0] {
            EnumChild::DataVariant { name, payload, .. } => {
                assert_eq!(name.0, "Ok");
                assert!(matches!(payload, TypeExpr::Param(p) if p.0 == "Output"));
            }
            _ => panic!("expected DataVariant"),
        }
        match &e.children[1] {
            EnumChild::DataVariant { name, payload, .. } => {
                assert_eq!(name.0, "Err");
                assert!(matches!(payload, TypeExpr::Param(p) if p.0 == "Failure"));
            }
            _ => panic!("expected DataVariant"),
        }
    }

    // ── Full v017 spec ──────────────────────────────────────

    #[test]
    fn parse_v017_spec() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../aski/spec/syntax-v017.aski");
        if !path.exists() { return; } // Skip if sibling repo not available
        let source = std::fs::read_to_string(&path).unwrap();
        let module = parse(&source);
        assert_eq!(module.name.0, "Elements");
        assert!(!module.enums.is_empty(), "expected enums, got 0");
        // v017 spec is a comprehensive test — check totals
        let total = module.enums.len() + module.structs.len()
            + module.newtypes.len() + module.consts.len()
            + module.trait_decls.len() + module.trait_impls.len();
        assert!(total >= 7, "expected at least 7 definitions, got {}", total);
    }

    #[test]
    fn parse_module_with_imports() {
        let module = parse("(App Element [Core Token ParseState])\n(Element Fire Earth Air Water)");
        assert_eq!(module.name.0, "App");
        assert_eq!(module.exports.len(), 1);
        assert!(!module.imports.is_empty(), "expected imports, got none");
        assert_eq!(module.imports[0].source.0, "Core");
        assert_eq!(module.imports[0].names.len(), 2);
    }

    // ── Trait implementations ───────────────────────────────

    #[test]
    #[ignore = "BUG: trait impl parsing not producing TraitImplDef"]
    fn parse_trait_impl_with_match_body() {
        let source = "\
(T E describe)
(E Fire Water)
(describe [(describe :@Self String)])
[describe E [
  (describe :@Self String (|
    (Fire) \"hot\"
    (Water) \"cold\"
  |))
]]";
        let module = parse(source);
        assert_eq!(module.trait_impls.len(), 1);
        let ti = &module.trait_impls[0];
        assert_eq!(ti.trait_name.0, "describe");
        assert_eq!(ti.methods.len(), 1);
        assert_eq!(ti.methods[0].name.0, "describe");
    }

    #[test]
    #[ignore = "BUG: trait impl parsing not producing TraitImplDef"]
    fn parse_trait_impl_with_block_body() {
        let source = "\
(T E compute)
{Addition (Left U32) (Right U32)}
(compute [(add :@Self U32)])
[compute Addition [
  (add :@Self U32 [
    @Self.Left + @Self.Right
  ])
]]";
        let module = parse(source);
        assert_eq!(module.trait_impls.len(), 1);
        let ti = &module.trait_impls[0];
        assert_eq!(ti.methods[0].name.0, "add");
        assert!(ti.methods[0].params.len() >= 1);
    }

    // ── Struct construction ─────────────────────────────────

    #[test]
    #[ignore = "BUG: struct construct method body not parsed"]
    fn parse_struct_construct_method() {
        let source = "\
(T E geom)
{Point (Horizontal F64) (Vertical F64)}
(geom [(offset :@Self @Delta Point Point)])
[geom Point [
  (offset :@Self @Delta Point Point
    {Point (Horizontal @Self.Horizontal + @Delta.Horizontal)
           (Vertical @Self.Vertical + @Delta.Vertical)})
]]";
        let module = parse(source);
        assert_eq!(module.trait_impls.len(), 1);
        assert_eq!(module.trait_impls[0].methods.len(), 1);
    }

    // ── FFI ─────────────────────────────────────────────────

    #[test]
    #[ignore = "BUG: FFI block not producing FfiDef"]
    fn parse_ffi_declaration() {
        let source = "\
(T E)
(| Lexer
  (lex @Source String [Vec Token])
|)";
        let module = parse(source);
        assert_eq!(module.ffi.len(), 1);
        let f = &module.ffi[0];
        assert_eq!(f.library.0, "Lexer");
        assert_eq!(f.functions.len(), 1);
        assert_eq!(f.functions[0].name.0, "lex");
    }

    // ── Constants ───────────────────────────────────────────

    #[test]
    fn parse_multiple_consts() {
        let source = "\
(T E)
{| MaxSigns U32 12 |}
{| Pi F64 3.14159 |}";
        let module = parse(source);
        assert_eq!(module.consts.len(), 2);
        assert_eq!(module.consts[0].name.0, "MaxSigns");
        assert_eq!(module.consts[1].name.0, "Pi");
    }

    // ── Process block ───────────────────────────────────────

    #[test]
    #[ignore = "BUG: process block not parsed"]
    fn parse_process_block() {
        let source = "\
(T E)
(E Fire Water)
[|
  @MyElement E/Fire
|]";
        let module = parse(source);
        assert!(module.process.is_some(), "expected process block");
    }

    // ── Newtype with application ────────────────────────────

    #[test]
    #[ignore = "BUG: multiple newtypes after other constructs not parsed"]
    fn parse_multiple_newtypes() {
        let source = "\
(T E)
Counter U32
Meters F64
Items [Vec String]";
        let module = parse(source);
        assert_eq!(module.newtypes.len(), 3);
        assert_eq!(module.newtypes[0].name.0, "Counter");
        assert_eq!(module.newtypes[1].name.0, "Meters");
        assert_eq!(module.newtypes[2].name.0, "Items");
    }

    // ── Nested struct inside struct ─────────────────────────

    #[test]
    fn parse_nested_struct_in_struct() {
        let source = "\
(T E)
{Drawing
  (Shapes [Vec String])
  Name
  {| Config (Timeout U32) (Retries U32) |}}";
        let module = parse(source);
        assert_eq!(module.structs.len(), 1);
        let s = &module.structs[0];
        assert_eq!(s.name.0, "Drawing");
        // Should have typed field, self-typed field, and nested struct
        assert!(s.children.len() >= 3);
    }

    // ── Module with action exports ──────────────────────────

    #[test]
    #[ignore = "BUG: camelCase trait exports not parsed correctly"]
    fn parse_module_with_trait_exports() {
        let source = "\
(T E describe compute)
(E Fire)
(describe [(describe :@Self String)])
(compute [(add :@Self U32)])";
        let module = parse(source);
        assert_eq!(module.exports.len(), 1); // E is type export
        // describe and compute should be trait exports
        // Note: the module grammar uses PascalCase for ObjectExport
        // and camelCase for actionExport
        assert_eq!(module.trait_decls.len(), 2);
    }

    // ── Early return ────────────────────────────────────────

    #[test]
    #[ignore = "BUG: early return in method body not parsed"]
    fn parse_early_return() {
        let source = "\
(T E lookup)
(E One Two)
(lookup [(find :@Self @Key String [Option String])])
[lookup E [
  (find :@Self @Key String [Option String] [
    ^E/One
  ])
]]";
        let module = parse(source);
        assert_eq!(module.trait_impls.len(), 1);
    }

    // ── Iteration ───────────────────────────────────────────

    #[test]
    #[ignore = "BUG: iteration in method body not parsed"]
    fn parse_iteration_body() {
        let source = "\
(T E iter)
(iter [(each :@Self)])
[iter E [
  (each :@Self {| @Self.Items.Item [@Item] |})
]]";
        let module = parse(source);
        assert_eq!(module.trait_impls.len(), 1);
    }

    // ── Multiple imports ────────────────────────────────────

    #[test]
    fn parse_multiple_import_blocks() {
        let source = "\
(App Element [Core Token] [Utils Helper])
(Element Fire)";
        let module = parse(source);
        assert_eq!(module.imports.len(), 2);
        assert_eq!(module.imports[0].source.0, "Core");
        assert_eq!(module.imports[1].source.0, "Utils");
    }

    #[test]
    fn debug_newtype_in_multi() {
        let source = "\
(Elements Element Quality)
(Element Fire Earth Air Water)
{Point (Horizontal F64) (Vertical F64)}
Counter U32
{| MaxSigns U32 12 |}";
        let module = parse(source);
        assert_eq!(module.newtypes.len(), 1);
        assert_eq!(module.consts.len(), 1);
    }

    #[test]
    #[ignore = "BUG: engine ordered choice — two consecutive newtypes produce 0. Newtype + Enum works. Root cause in engine's match_repeated_choice"]
    fn parse_two_newtypes() {
        let module = parse("(T E)\nCounter U32\nMeters F64");
        assert_eq!(module.newtypes.len(), 2);
    }
}
