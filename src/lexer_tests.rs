#[cfg(test)]
mod tests {
    use crate::lexer::*;

    #[test]
    fn lex_enum_declaration() {
        let source = "(Element Fire Earth Air Water)";
        let tokens = lex(source).unwrap();
        let kinds: Vec<_> = tokens.iter().map(|t| &t.token).collect();
        assert_eq!(
            kinds,
            vec![
                &Token::LParen,
                &Token::PascalIdent("Element".into()),
                &Token::PascalIdent("Fire".into()),
                &Token::PascalIdent("Earth".into()),
                &Token::PascalIdent("Air".into()),
                &Token::PascalIdent("Water".into()),
                &Token::RParen,
            ]
        );
    }

    #[test]
    fn lex_colon_borrow() {
        let source = ":@Self";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::Colon);
        assert_eq!(tokens[1].token, Token::At);
        assert_eq!(tokens[2].token, Token::PascalIdent("Self".into()));
    }

    #[test]
    fn lex_struct_declaration() {
        let source = "{Point (X F64) (Y F64)}";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::LBrace);
        assert_eq!(tokens[1].token, Token::PascalIdent("Point".into()));
        assert_eq!(tokens[2].token, Token::LParen);
        assert_eq!(tokens[3].token, Token::PascalIdent("X".into()));
        assert_eq!(tokens[4].token, Token::PascalIdent("F64".into()));
        assert_eq!(tokens[5].token, Token::RParen);
    }

    #[test]
    fn lex_comment_stripped() {
        let source = ";; this is a comment\n(Element Fire)";
        let tokens = lex(source).unwrap();
        assert!(!tokens.iter().any(|t| matches!(t.token, Token::Comment)));
        assert!(tokens
            .iter()
            .any(|t| t.token == Token::PascalIdent("Element".into())));
    }

    #[test]
    fn lex_string_literal() {
        let source = r#""hello world""#;
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::StringLit("hello world".into()));
    }

    #[test]
    fn lex_numeric_literals() {
        let source = "42 3.14";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::Integer(42));
        assert_eq!(tokens[1].token, Token::Float("3.14".into()));
    }

    #[test]
    fn lex_const_declaration() {
        let source = "{| Pi F64 3.14159265358979 |}";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::LBracePipe);
        assert_eq!(tokens[1].token, Token::PascalIdent("Pi".into()));
    }

    #[test]
    fn lex_piped_delimiters() {
        let source = "(| Match |)";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::LParenPipe);
        assert_eq!(tokens[1].token, Token::PascalIdent("Match".into()));
        assert_eq!(tokens[2].token, Token::RPipeParen);
    }

    #[test]
    fn lex_type_application() {
        let source = "[Vec Element]";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::LBracket);
        assert_eq!(tokens[1].token, Token::PascalIdent("Vec".into()));
        assert_eq!(tokens[2].token, Token::PascalIdent("Element".into()));
        assert_eq!(tokens[3].token, Token::RBracket);
    }

    #[test]
    fn lex_path_syntax() {
        let source = "Element/Fire";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::PascalIdent("Element".into()));
        assert_eq!(tokens[1].token, Token::Slash);
        assert_eq!(tokens[2].token, Token::PascalIdent("Fire".into()));
    }

    #[test]
    fn lex_generic_param() {
        let source = "$Clone&Debug";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::Dollar);
        assert_eq!(tokens[1].token, Token::PascalIdent("Clone".into()));
        assert_eq!(tokens[2].token, Token::Ampersand);
        assert_eq!(tokens[3].token, Token::PascalIdent("Debug".into()));
    }

    #[test]
    fn lex_instance_declaration() {
        let source = "@Counter U32/new(0)";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::At);
        assert_eq!(tokens[1].token, Token::PascalIdent("Counter".into()));
        assert_eq!(tokens[2].token, Token::PascalIdent("U32".into()));
        assert_eq!(tokens[3].token, Token::Slash);
        assert_eq!(tokens[4].token, Token::CamelIdent("new".into()));
        assert_eq!(tokens[5].token, Token::LParen);
        assert_eq!(tokens[6].token, Token::Integer(0));
        assert_eq!(tokens[7].token, Token::RParen);
    }

    #[test]
    fn lex_early_return() {
        let source = "^Option/None";
        let tokens = lex(source).unwrap();
        assert_eq!(tokens[0].token, Token::Caret);
        assert_eq!(tokens[1].token, Token::PascalIdent("Option".into()));
        assert_eq!(tokens[2].token, Token::Slash);
        assert_eq!(tokens[3].token, Token::PascalIdent("None".into()));
    }
}
