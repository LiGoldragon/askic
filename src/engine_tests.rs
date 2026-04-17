#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::engine::Engine;
    use aski::*;

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
}
