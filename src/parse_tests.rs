#[cfg(test)]
mod tests {
    use crate::parse::parse_source;
    use crate::domain::*;

    #[test]
    fn parse_simple_enum() {
        let prog = parse_source("(Test Element)\n(Element Fire Earth Air Water)").unwrap();
        assert_eq!(prog.children.len(), 2); // module + enum
        match &prog.children[1] {
            RootChild::Enum(e) => {
                assert_eq!(e.name.0, "Element");
                assert_eq!(e.children.len(), 4);
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn parse_struct() {
        let prog = parse_source("(Test Point)\n{Point (Horizontal F64) (Vertical F64)}").unwrap();
        match &prog.children[1] {
            RootChild::Struct(s) => {
                assert_eq!(s.name.0, "Point");
                assert_eq!(s.children.len(), 2);
            }
            other => panic!("expected Struct, got {:?}", other),
        }
    }

    #[test]
    fn parse_newtype() {
        let prog = parse_source("(Test Counter)\nCounter U32").unwrap();
        match &prog.children[1] {
            RootChild::Newtype(n) => {
                assert_eq!(n.name.0, "Counter");
                match &n.wraps {
                    TypeExpr::Simple(name) => assert_eq!(name.0, "U32"),
                    other => panic!("expected Simple type, got {:?}", other),
                }
            }
            other => panic!("expected Newtype, got {:?}", other),
        }
    }

    #[test]
    fn parse_const() {
        let prog = parse_source("(Test Pi)\n{| Pi F64 3.14 |}").unwrap();
        match &prog.children[1] {
            RootChild::Const(c) => {
                assert_eq!(c.name.0, "Pi");
                match &c.value {
                    LiteralValue::Float(v) => assert!(*v > 3.0),
                    other => panic!("expected Float, got {:?}", other),
                }
            }
            other => panic!("expected Const, got {:?}", other),
        }
    }

    #[test]
    fn parse_trait_decl() {
        let prog = parse_source("(Test describe)\n(describe [(describe :@Self Quality)])").unwrap();
        match &prog.children[1] {
            RootChild::TraitDecl(t) => {
                assert_eq!(t.name.0, "describe");
                assert_eq!(t.signatures.len(), 1);
                assert_eq!(t.signatures[0].name.0, "describe");
            }
            other => panic!("expected TraitDecl, got {:?}", other),
        }
    }

    #[test]
    fn parse_trait_impl_with_match() {
        let source = r#"
(Test Element Quality describe)
(Element Fire Earth Air Water)
(Quality Passionate Grounded Intellectual Intuitive)
(describe [(describe :@Self Quality)])
[describe Element [
  (describe :@Self Quality (|
    (Fire) Passionate
    (Earth) Grounded
    (Air) Intellectual
    (Water) Intuitive
  |))
]]
"#;
        let prog = parse_source(source).unwrap();
        // module + Element + Quality + describe decl + describe impl
        assert_eq!(prog.children.len(), 5);
        match &prog.children[4] {
            RootChild::TraitImpl(i) => {
                assert_eq!(i.trait_name.0, "describe");
                assert_eq!(i.type_name.0, "Element");
                assert_eq!(i.methods.len(), 1);
            }
            other => panic!("expected TraitImpl, got {:?}", other),
        }
    }

    #[test]
    fn parse_block_body() {
        let source = r#"
(Test Addition compute)
{Addition (Left U32) (Right U32)}
(compute [(add :@Self U32)])
[compute Addition [
  (add :@Self U32 [
    @Self.Left + @Self.Right
  ])
]]
"#;
        let prog = parse_source(source).unwrap();
        assert!(prog.children.len() >= 4);
    }

    #[test]
    fn parse_process_block() {
        let source = r#"
(Test Element describe)
(Element Fire Earth Air Water)
[|
  @MyElement Element/Fire
  StdOut/print(@MyElement.describe)
|]
"#;
        let prog = parse_source(source).unwrap();
        let has_process = prog.children.iter().any(|c| matches!(c, RootChild::Process(_)));
        assert!(has_process);
    }
}
