#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::engine::Engine;
    use sema_core::*;

    fn parse(source: &str) -> Vec<RootChild> {
        let tokens = lex(source).expect("lex failed");
        let data = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../askicc/generated/dialects.rkyv")
        ).expect("dialect data not found — run askicc first");
        let data: &'static [u8] = Box::leak(data.into_boxed_slice());
        let engine = Engine::new(data);
        engine.parse(&tokens).expect("parse failed")
    }

    // ── Module ──────────────────────────────────────────────

    #[test]
    fn parse_module_only() {
        let children = parse("(Elements Element Quality)");
        assert_eq!(children.len(), 1);
        match &children[0] {
            RootChild::Module(m) => {
                assert_eq!(m.name.0, "Elements");
                assert_eq!(m.exports.len(), 2);
            }
            _ => panic!("expected Module"),
        }
    }

    // ── Enum ────────────────────────────────────────────────

    #[test]
    fn parse_bare_enum() {
        let children = parse("(T E)\n(Element Fire Earth Air Water)");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::Enum(e) => {
                assert_eq!(e.name.0, "Element");
                assert_eq!(e.children.len(), 4);
                match &e.children[0] {
                    EnumChild::Variant { name, .. } => assert_eq!(name.0, "Fire"),
                    _ => panic!("expected Variant"),
                }
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn parse_data_carrying_enum() {
        let children = parse("(T E)\n(Option (Some String) None)");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::Enum(e) => {
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
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn parse_type_application_in_enum() {
        let children = parse("(T E)\n(ParseResult (Success [Vec Node]) (Failure String))");
        match &children[1] {
            RootChild::Enum(e) => {
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
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn parse_nested_enum() {
        let children = parse(
            "(T E)\n(Token (Ident String) (Number I64) (| Delimiter LParen RParen |) Newline)"
        );
        match &children[1] {
            RootChild::Enum(e) => {
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
            _ => panic!("expected Enum"),
        }
    }

    // ── Struct ──────────────────────────────────────────────

    #[test]
    fn parse_struct() {
        let children = parse("(T E)\n{Point (Horizontal F64) (Vertical F64)}");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::Struct(s) => {
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
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn parse_self_typed_field() {
        let children = parse("(T E)\n{Drawing (Shapes [Vec Shape]) Name}");
        match &children[1] {
            RootChild::Struct(s) => {
                assert_eq!(s.children.len(), 2);
                match &s.children[1] {
                    StructChild::SelfTypedField { name, .. } => assert_eq!(name.0, "Name"),
                    _ => panic!("expected SelfTypedField"),
                }
            }
            _ => panic!("expected Struct"),
        }
    }

    // ── Newtype ─────────────────────────────────────────────

    #[test]
    fn parse_newtype() {
        let children = parse("(T E)\nCounter U32");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::Newtype(n) => {
                assert_eq!(n.name.0, "Counter");
                assert!(matches!(&n.wraps, TypeExpr::Named(t) if t.0 == "U32"));
            }
            _ => panic!("expected Newtype"),
        }
    }

    #[test]
    fn parse_newtype_with_type_application() {
        let children = parse("(T E)\nItems [Vec Item]");
        match &children[1] {
            RootChild::Newtype(n) => {
                assert_eq!(n.name.0, "Items");
                match &n.wraps {
                    TypeExpr::Application(ta) => {
                        assert_eq!(ta.constructor.0, "Vec");
                    }
                    _ => panic!("expected Application"),
                }
            }
            _ => panic!("expected Newtype"),
        }
    }

    // ── Const ───────────────────────────────────────────────

    #[test]
    fn parse_const_int() {
        let children = parse("(T E)\n{| MaxSigns U32 12 |}");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::Const(c) => {
                assert_eq!(c.name.0, "MaxSigns");
                assert!(matches!(&c.typ, TypeExpr::Named(t) if t.0 == "U32"));
                assert!(matches!(&c.value, LiteralValue::Int(12)));
            }
            _ => panic!("expected Const"),
        }
    }

    #[test]
    fn parse_const_float() {
        let children = parse("(T E)\n{| Pi F64 3.14 |}");
        match &children[1] {
            RootChild::Const(c) => {
                assert_eq!(c.name.0, "Pi");
                match &c.value {
                    LiteralValue::Float(v) => assert!((v - 3.14).abs() < 0.001),
                    _ => panic!("expected Float"),
                }
            }
            _ => panic!("expected Const"),
        }
    }

    // ── Trait Declaration ───────────────────────────────────

    #[test]
    fn parse_trait_decl() {
        let children = parse("(T E describe)\n(describe [(describe :@Self Quality)])");
        assert_eq!(children.len(), 2);
        match &children[1] {
            RootChild::TraitDecl(td) => {
                assert_eq!(td.name.0, "describe");
                assert_eq!(td.signatures.len(), 1);
                assert_eq!(td.signatures[0].name.0, "describe");
            }
            _ => panic!("expected TraitDecl"),
        }
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
        let children = parse(source);
        assert_eq!(children.len(), 6);
        assert!(matches!(&children[0], RootChild::Module(_)));
        assert!(matches!(&children[1], RootChild::Enum(_)));
        assert!(matches!(&children[2], RootChild::Enum(_)));
        assert!(matches!(&children[3], RootChild::Struct(_)));
        assert!(matches!(&children[4], RootChild::Newtype(_)));
        assert!(matches!(&children[5], RootChild::Const(_)));
    }

    // ── Generic types ───────────────────────────────────────

    #[test]
    fn parse_generic_enum() {
        let children = parse("(T E)\n(Option (Some $Value) None)");
        match &children[1] {
            RootChild::Enum(e) => {
                match &e.children[0] {
                    EnumChild::DataVariant { payload, .. } => {
                        assert!(matches!(payload, TypeExpr::Param(p) if p.0 == "Value"));
                    }
                    _ => panic!("expected DataVariant"),
                }
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn parse_two_param_generic() {
        let children = parse("(T E)\n(Result (Ok $Output) (Err $Failure))");
        match &children[1] {
            RootChild::Enum(e) => {
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
            _ => panic!("expected Enum"),
        }
    }

    // ── Full v017 spec ──────────────────────────────────────

    #[test]
    fn parse_v017_spec() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../aski/spec/syntax-v017.aski")
        );
        if let Ok(source) = source {
            let children = parse(&source);
            assert!(children.len() >= 7, "expected at least 7 root children, got {}", children.len());
            assert!(matches!(&children[0], RootChild::Module(_)));
        }
        // Skip if file not found (CI without sibling repos)
    }
}
