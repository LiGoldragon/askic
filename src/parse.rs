/// askic parser — recursive descent following synth rules.
///
/// Reads .aski source into populated domain trees.
/// Allocates into Arena — all references are typed indices.

use crate::lexer::{Token, Spanned, lex};
use crate::domain::*;

pub struct Parser<'a> {
    tokens: &'a [Spanned],
    pos: usize,
    pub arena: Arena,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned]) -> Self {
        Parser { tokens, pos: 0, arena: Arena::default() }
    }

    pub fn parse_program(&mut self) -> Result<AskiProgram, String> {
        let mut children = Vec::new();
        let module = self.parse_module_decl()?;
        children.push(RootChild::Module(module));
        while !self.at_end() {
            let before = self.pos;
            children.push(self.parse_root_child()?);
            if self.pos <= before { return Err(format!("stuck at root pos {}", self.pos)); }
        }
        Ok(AskiProgram { children, arena: std::mem::take(&mut self.arena) })
    }

    fn peek(&self) -> Option<&Token> {
        let mut i = self.pos;
        while i < self.tokens.len() {
            if !matches!(self.tokens[i].token, Token::Newline) { return Some(&self.tokens[i].token); }
            i += 1;
        }
        None
    }

    fn advance(&mut self) -> Option<&Token> {
        self.skip_newlines();
        let tok = self.tokens.get(self.pos).map(|t| &t.token);
        if tok.is_some() { self.pos += 1; }
        tok
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() && matches!(self.tokens[self.pos].token, Token::Newline) { self.pos += 1; }
    }

    fn at_end(&self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() { if !matches!(self.tokens[i].token, Token::Newline) { return false; } i += 1; }
        true
    }

    fn span_here(&self) -> Span {
        let mut i = self.pos;
        while i < self.tokens.len() && matches!(self.tokens[i].token, Token::Newline) { i += 1; }
        if i < self.tokens.len() { Span { start: self.tokens[i].span.start as u32, end: self.tokens[i].span.end as u32 } }
        else { Span { start: 0, end: 0 } }
    }

    fn span_from(&self, start: u32) -> Span {
        let end = if self.pos > 0 { self.tokens[self.pos - 1].span.end as u32 } else { start };
        Span { start, end }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let pos = self.pos;
        let got = self.advance().cloned();
        if got.as_ref() == Some(expected) { Ok(()) }
        else { Err(format!("expected {:?}, got {:?} at pos {}", expected, got, pos)) }
    }

    fn expect_pascal(&mut self) -> Result<Name, String> {
        let pos = self.pos;
        match self.advance().cloned() {
            Some(Token::PascalIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected PascalCase, got {:?} at pos {}", other, pos)),
        }
    }

    fn expect_camel(&mut self) -> Result<Name, String> {
        let pos = self.pos;
        match self.advance().cloned() {
            Some(Token::CamelIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected camelCase, got {:?} at pos {}", other, pos)),
        }
    }

    fn expect_name(&mut self) -> Result<Name, String> {
        let pos = self.pos;
        match self.advance().cloned() {
            Some(Token::PascalIdent(s)) | Some(Token::CamelIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected name, got {:?} at pos {}", other, pos)),
        }
    }

    fn parse_module_decl(&mut self) -> Result<ModuleDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_pascal()?;
        let mut exports = Vec::new();
        let mut imports = Vec::new();
        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in module".into()); }
            if self.peek() == Some(&Token::LBracket) {
                self.expect(&Token::LBracket)?;
                let source = self.expect_pascal()?;
                let mut names = Vec::new();
                while self.peek() != Some(&Token::RBracket) { names.push(self.expect_name()?); }
                self.expect(&Token::RBracket)?;
                imports.push(ModuleImport { source, names });
            } else { exports.push(self.expect_name()?); }
        }
        self.expect(&Token::RParen)?;
        Ok(ModuleDef { name, exports, imports, span: self.span_from(start) })
    }

    fn parse_root_child(&mut self) -> Result<RootChild, String> {
        match self.peek() {
            Some(Token::LParen) => self.parse_root_paren(),
            Some(Token::LBracket) => self.parse_trait_impl(),
            Some(Token::LBrace) => { let s = self.parse_struct_def()?; Ok(RootChild::Struct(s)) }
            Some(Token::LBracePipe) => self.parse_const(),
            Some(Token::LParenPipe) => self.parse_ffi(),
            Some(Token::LBracketPipe) => self.parse_process(),
            Some(Token::PascalIdent(_)) => { let n = self.parse_newtype_def()?; Ok(RootChild::Newtype(n)) }
            other => Err(format!("expected root construct, got {:?}", other)),
        }
    }

    fn parse_root_paren(&mut self) -> Result<RootChild, String> {
        self.expect(&Token::LParen)?;
        match self.peek() {
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                let start = self.span_here().start;
                let mut refs = Vec::new();
                while self.peek() != Some(&Token::RParen) {
                    if self.at_end() { return Err("unexpected EOF in enum".into()); }
                    let before = self.pos;
                    let c = self.parse_enum_variant()?;
                    refs.push(self.arena.alloc_enum_child(c));
                    if self.pos <= before { return Err("stuck in enum".into()); }
                }
                self.expect(&Token::RParen)?;
                Ok(RootChild::Enum(EnumDef { name, children: refs, span: self.span_from(start) }))
            }
            Some(Token::CamelIdent(_)) => {
                let name = self.expect_camel()?;
                let start = self.span_here().start;
                self.expect(&Token::LBracket)?;
                let mut sigs = Vec::new();
                while self.peek() != Some(&Token::RBracket) {
                    if self.at_end() { return Err("unexpected EOF in trait decl".into()); }
                    sigs.push(self.parse_method_sig()?);
                }
                self.expect(&Token::RBracket)?;
                self.expect(&Token::RParen)?;
                Ok(RootChild::TraitDecl(TraitDeclDef { name, signatures: sigs, span: self.span_from(start) }))
            }
            other => Err(format!("expected name after (, got {:?}", other)),
        }
    }

    fn parse_enum_variant(&mut self) -> Result<EnumChild, String> {
        let start = self.span_here().start;
        match self.peek() {
            Some(Token::LParen) => {
                self.expect(&Token::LParen)?;
                let name = self.expect_pascal()?;
                if self.peek() == Some(&Token::RParen) {
                    self.expect(&Token::RParen)?;
                    Ok(EnumChild::Variant { name, span: self.span_from(start) })
                } else {
                    let payload = self.parse_type_expr()?;
                    self.expect(&Token::RParen)?;
                    Ok(EnumChild::DataVariant { name, payload, span: self.span_from(start) })
                }
            }
            Some(Token::LBrace) => {
                let s = self.parse_struct_def()?;
                let fields: Vec<StructField> = s.children.iter().map(|r| {
                    match &self.arena.struct_children[r.idx()] {
                        StructChild::TypedField { name, typ, span } => StructField { name: name.clone(), typ: typ.clone(), span: span.clone() },
                        StructChild::SelfTypedField { name, span } => StructField { name: name.clone(), typ: TypeExpr::Simple(name.clone()), span: span.clone() },
                        _ => panic!("nested def in struct variant"),
                    }
                }).collect();
                Ok(EnumChild::StructVariant { name: s.name, fields, span: s.span })
            }
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                Ok(EnumChild::Variant { name, span: self.span_from(start) })
            }
            other => Err(format!("expected variant, got {:?}", other)),
        }
    }

    fn parse_struct_def(&mut self) -> Result<StructDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBrace)?;
        let name = self.expect_pascal()?;
        let mut refs = Vec::new();
        while self.peek() != Some(&Token::RBrace) {
            if self.at_end() { return Err("unexpected EOF in struct".into()); }
            let before = self.pos;
            let c = self.parse_struct_child()?;
            refs.push(self.arena.alloc_struct_child(c));
            if self.pos <= before { return Err("stuck in struct".into()); }
        }
        self.expect(&Token::RBrace)?;
        Ok(StructDef { name, children: refs, span: self.span_from(start) })
    }

    fn parse_struct_child(&mut self) -> Result<StructChild, String> {
        let start = self.span_here().start;
        match self.peek() {
            Some(Token::LParen) => {
                self.expect(&Token::LParen)?;
                let name = self.expect_pascal()?;
                let typ = self.parse_type_expr()?;
                self.expect(&Token::RParen)?;
                Ok(StructChild::TypedField { name, typ, span: self.span_from(start) })
            }
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                Ok(StructChild::SelfTypedField { name, span: self.span_from(start) })
            }
            other => Err(format!("expected struct field, got {:?}", other)),
        }
    }

    fn parse_newtype_def(&mut self) -> Result<NewtypeDef, String> {
        let start = self.span_here().start;
        let name = self.expect_pascal()?;
        let wraps = self.parse_type_expr()?;
        Ok(NewtypeDef { name, wraps, span: self.span_from(start) })
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, String> {
        match self.peek() {
            Some(Token::LBracket) => {
                self.expect(&Token::LBracket)?;
                let constructor = self.expect_pascal()?;
                let mut args = Vec::new();
                while self.peek() != Some(&Token::RBracket) {
                    if self.at_end() { return Err("unexpected EOF in type app".into()); }
                    let before = self.pos;
                    args.push(self.parse_type_expr()?);
                    if self.pos <= before { return Err("stuck in type app".into()); }
                }
                self.expect(&Token::RBracket)?;
                Ok(TypeExpr::Application { constructor, args })
            }
            Some(Token::Dollar) => {
                self.advance();
                let first = self.expect_pascal()?;
                let mut bounds = vec![first];
                while self.peek() == Some(&Token::Ampersand) { self.advance(); bounds.push(self.expect_pascal()?); }
                if bounds.len() == 1 { Ok(TypeExpr::Param(bounds.remove(0))) }
                else { Ok(TypeExpr::BoundedParam { bounds }) }
            }
            Some(Token::PascalIdent(_)) => { let name = self.expect_pascal()?; Ok(TypeExpr::Simple(name)) }
            other => Err(format!("expected type, got {:?}", other)),
        }
    }

    fn parse_trait_impl(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBracket)?;
        let trait_name = self.expect_camel()?;
        let type_name = self.expect_pascal()?;
        self.expect(&Token::LBracket)?;
        let mut refs = Vec::new();
        while self.peek() != Some(&Token::RBracket) {
            if self.at_end() { return Err("unexpected EOF in impl".into()); }
            let md = self.parse_method_def()?;
            refs.push(self.arena.alloc_method_def(md));
        }
        self.expect(&Token::RBracket)?;
        self.expect(&Token::RBracket)?;
        Ok(RootChild::TraitImpl(TraitImplDef { trait_name, type_name, methods: refs, span: self.span_from(start) }))
    }

    fn parse_method_sig(&mut self) -> Result<MethodSig, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut prefs = Vec::new();
        let mut ret = None;
        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in sig".into()); }
            match self.peek() {
                Some(Token::Colon) | Some(Token::Tilde) | Some(Token::At) => {
                    let p = self.parse_param()?; prefs.push(self.arena.alloc_param(p));
                }
                _ => { ret = Some(self.parse_type_expr()?); break; }
            }
        }
        self.expect(&Token::RParen)?;
        Ok(MethodSig { name, params: prefs, return_type: ret, span: self.span_from(start) })
    }

    fn parse_method_def(&mut self) -> Result<MethodDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut prefs = Vec::new();
        let mut ret = None;
        loop {
            match self.peek() {
                Some(Token::Colon) | Some(Token::Tilde) | Some(Token::At) => {
                    let p = self.parse_param()?; prefs.push(self.arena.alloc_param(p));
                }
                Some(Token::LBracket) | Some(Token::LParenPipe) | Some(Token::LBracketPipe)
                | Some(Token::LBracePipe) | Some(Token::LBrace) | Some(Token::RParen) => break,
                _ => { ret = Some(self.parse_type_expr()?); }
            }
        }
        let body = if self.peek() == Some(&Token::RParen) {
            let b = self.arena.alloc_block(Block { stmts: vec![], tail: None });
            MethodBody::Block(b)
        } else { self.parse_method_body()? };
        self.expect(&Token::RParen)?;
        Ok(MethodDef { name, params: prefs, return_type: ret, body, span: self.span_from(start) })
    }

    fn parse_param(&mut self) -> Result<Param, String> {
        match self.peek() {
            Some(Token::Colon) => {
                self.advance(); self.expect(&Token::At)?;
                if self.peek() == Some(&Token::PascalIdent("Self".into())) { self.advance(); Ok(Param::BorrowSelf) }
                else { let n = self.expect_pascal()?; let t = self.parse_type_expr()?; Ok(Param::Named { name: n, typ: t }) }
            }
            Some(Token::Tilde) => {
                self.advance(); self.expect(&Token::At)?;
                if self.peek() == Some(&Token::PascalIdent("Self".into())) { self.advance(); Ok(Param::MutBorrowSelf) }
                else { let n = self.expect_pascal()?; let t = self.parse_type_expr()?; Ok(Param::Named { name: n, typ: t }) }
            }
            Some(Token::At) => {
                self.advance();
                if self.peek() == Some(&Token::PascalIdent("Self".into())) { self.advance(); Ok(Param::OwnedSelf) }
                else {
                    let n = self.expect_pascal()?;
                    if matches!(self.peek(), Some(Token::PascalIdent(_)) | Some(Token::LBracket) | Some(Token::Dollar)) {
                        let t = self.parse_type_expr()?; Ok(Param::Named { name: n, typ: t })
                    } else { Ok(Param::Bare { name: n }) }
                }
            }
            other => Err(format!("expected param, got {:?}", other)),
        }
    }

    fn parse_method_body(&mut self) -> Result<MethodBody, String> {
        match self.peek() {
            Some(Token::LBracket) => { let b = self.parse_block()?; Ok(MethodBody::Block(b)) }
            Some(Token::LParenPipe) => { let m = self.parse_match_expr()?; Ok(MethodBody::Match(m)) }
            Some(Token::LBracketPipe) => { let b = self.parse_loop_block()?; Ok(MethodBody::Loop(b)) }
            Some(Token::LBracePipe) => { let (s, b) = self.parse_iteration()?; Ok(MethodBody::Iteration { source: s, body: b }) }
            Some(Token::LBrace) => {
                let start = self.span_here().start;
                self.expect(&Token::LBrace)?;
                let typ = self.expect_pascal()?;
                let mut refs = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    if self.at_end() { return Err("unexpected EOF in struct construct".into()); }
                    self.expect(&Token::LParen)?;
                    let name = self.expect_pascal()?;
                    let value = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    refs.push(self.arena.alloc_field_init(FieldInit { name, value }));
                }
                self.expect(&Token::RBrace)?;
                Ok(MethodBody::StructConstruct { typ, fields: refs, span: self.span_from(start) })
            }
            other => Err(format!("expected method body, got {:?}", other)),
        }
    }

    fn parse_block(&mut self) -> Result<BlockRef, String> {
        self.expect(&Token::LBracket)?;
        let mut srefs = Vec::new();
        let mut tail = None;
        while self.peek() != Some(&Token::RBracket) {
            if self.at_end() { return Err("unexpected EOF in block".into()); }
            let before = self.pos;
            let stmt = self.parse_statement()?;
            if self.peek() == Some(&Token::RBracket) {
                if let Statement::Expr(e) = stmt { tail = Some(e); break; }
                else { srefs.push(self.arena.alloc_stmt(stmt)); }
            } else { srefs.push(self.arena.alloc_stmt(stmt)); }
            if self.pos <= before { return Err("stuck in block".into()); }
        }
        self.expect(&Token::RBracket)?;
        Ok(self.arena.alloc_block(Block { stmts: srefs, tail }))
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek() {
            Some(Token::Caret) => { self.advance(); let e = self.parse_expr()?; Ok(Statement::EarlyReturn(e)) }
            Some(Token::LBracketPipe) => { let b = self.parse_loop_block()?; Ok(Statement::Loop(b)) }
            Some(Token::LBracePipe) => { let (s, b) = self.parse_iteration()?; Ok(Statement::Iteration { source: s, body: b }) }
            Some(Token::LParen) => {
                let start = self.span_here().start;
                self.expect(&Token::LParen)?;
                let name = self.expect_pascal()?;
                let typ = self.parse_type_expr()?;
                self.expect(&Token::RParen)?;
                Ok(Statement::LocalTypeDecl { name, typ, span: self.span_from(start) })
            }
            Some(Token::Tilde) => {
                self.advance(); self.expect(&Token::At)?;
                let name = self.expect_pascal()?;
                if self.peek() == Some(&Token::Dot) {
                    self.advance();
                    let method = self.expect_camel()?;
                    self.expect(&Token::LParen)?;
                    let mut args = Vec::new();
                    while self.peek() != Some(&Token::RParen) { args.push(self.parse_expr()?); }
                    self.expect(&Token::RParen)?;
                    Ok(Statement::Mutation { name, method, args, span: self.span_here() })
                } else {
                    let v = self.parse_expr()?;
                    Ok(Statement::Instance { name, type_annotation: None, value: v, span: self.span_here() })
                }
            }
            Some(Token::At) => {
                let saved = self.pos;
                self.advance();
                let name = self.expect_pascal()?;
                match self.peek() {
                    Some(Token::Dot) | Some(Token::Question) | Some(Token::Plus) | Some(Token::Minus)
                    | Some(Token::Star) | Some(Token::Percent) | Some(Token::DoubleEquals)
                    | Some(Token::NotEqual) | Some(Token::LessThan) | Some(Token::GreaterThan)
                    | Some(Token::LessThanOrEqual) | Some(Token::GreaterThanOrEqual)
                    | Some(Token::LogicalAnd) | Some(Token::LogicalOr) => {
                        self.pos = saved;
                        let e = self.parse_expr()?;
                        return Ok(Statement::Expr(e));
                    }
                    _ => {}
                }
                let ta = if self.peek() == Some(&Token::LParen) {
                    let s = self.pos;
                    self.expect(&Token::LParen)?;
                    match self.parse_type_expr() {
                        Ok(t) if self.peek() == Some(&Token::RParen) => { self.expect(&Token::RParen)?; Some(t) }
                        _ => { self.pos = s; None }
                    }
                } else { None };
                let v = self.parse_expr()?;
                Ok(Statement::Instance { name, type_annotation: ta, value: v, span: self.span_here() })
            }
            _ => { let e = self.parse_expr()?; Ok(Statement::Expr(e)) }
        }
    }

    fn parse_expr(&mut self) -> Result<ExprRef, String> { self.parse_expr_or() }

    fn parse_expr_or(&mut self) -> Result<ExprRef, String> {
        let mut l = self.parse_expr_and()?;
        while self.peek() == Some(&Token::LogicalOr) {
            let s = self.span_here(); self.advance(); let r = self.parse_expr_and()?;
            l = self.arena.alloc_expr(Expr::BinOr { left: l, right: r, span: s });
        }
        Ok(l)
    }

    fn parse_expr_and(&mut self) -> Result<ExprRef, String> {
        let mut l = self.parse_expr_compare()?;
        while self.peek() == Some(&Token::LogicalAnd) {
            let s = self.span_here(); self.advance(); let r = self.parse_expr_compare()?;
            l = self.arena.alloc_expr(Expr::BinAnd { left: l, right: r, span: s });
        }
        Ok(l)
    }

    fn parse_expr_compare(&mut self) -> Result<ExprRef, String> {
        let l = self.parse_expr_add()?;
        let s = self.span_here();
        match self.peek() {
            Some(Token::DoubleEquals) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinEq { left: l, right: r, span: s })) }
            Some(Token::NotEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinNotEq { left: l, right: r, span: s })) }
            Some(Token::LessThan) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinLt { left: l, right: r, span: s })) }
            Some(Token::GreaterThan) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinGt { left: l, right: r, span: s })) }
            Some(Token::LessThanOrEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinLtEq { left: l, right: r, span: s })) }
            Some(Token::GreaterThanOrEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(self.arena.alloc_expr(Expr::BinGtEq { left: l, right: r, span: s })) }
            _ => Ok(l),
        }
    }

    fn parse_expr_add(&mut self) -> Result<ExprRef, String> {
        let mut l = self.parse_expr_mul()?;
        loop { let s = self.span_here(); match self.peek() {
            Some(Token::Plus) => { self.advance(); let r = self.parse_expr_mul()?; l = self.arena.alloc_expr(Expr::BinAdd { left: l, right: r, span: s }); }
            Some(Token::Minus) => { self.advance(); let r = self.parse_expr_mul()?; l = self.arena.alloc_expr(Expr::BinSub { left: l, right: r, span: s }); }
            _ => break,
        }}
        Ok(l)
    }

    fn parse_expr_mul(&mut self) -> Result<ExprRef, String> {
        let mut l = self.parse_expr_postfix()?;
        loop { let s = self.span_here(); match self.peek() {
            Some(Token::Star) => { self.advance(); let r = self.parse_expr_postfix()?; l = self.arena.alloc_expr(Expr::BinMul { left: l, right: r, span: s }); }
            Some(Token::Percent) => { self.advance(); let r = self.parse_expr_postfix()?; l = self.arena.alloc_expr(Expr::BinMod { left: l, right: r, span: s }); }
            _ => break,
        }}
        Ok(l)
    }

    fn parse_expr_postfix(&mut self) -> Result<ExprRef, String> {
        let mut e = self.parse_expr_atom()?;
        loop { match self.peek() {
            Some(Token::Dot) => {
                let s = self.span_here(); self.advance();
                match self.peek() {
                    Some(Token::PascalIdent(_)) => { let f = self.expect_pascal()?; e = self.arena.alloc_expr(Expr::FieldAccess { object: e, field: f, span: s }); }
                    Some(Token::CamelIdent(_)) => {
                        let m = self.expect_camel()?;
                        let mut args = Vec::new();
                        if self.peek() == Some(&Token::LParen) {
                            self.advance();
                            while self.peek() != Some(&Token::RParen) { args.push(self.parse_expr()?); }
                            self.expect(&Token::RParen)?;
                        }
                        e = self.arena.alloc_expr(Expr::MethodCall { object: e, method: m, args, span: s });
                    }
                    other => return Err(format!("expected field/method, got {:?}", other)),
                }
            }
            Some(Token::Question) => { let s = self.span_here(); self.advance(); e = self.arena.alloc_expr(Expr::TryUnwrap { inner: e, span: s }); }
            _ => break,
        }}
        Ok(e)
    }

    fn parse_expr_atom(&mut self) -> Result<ExprRef, String> {
        let s = self.span_here();
        match self.peek() {
            Some(Token::At) => {
                self.advance(); let n = self.expect_pascal()?;
                Ok(self.arena.alloc_expr(Expr::InstanceRef { name: n, span: s }))
            }
            Some(Token::PascalIdent(_)) => {
                let n = self.expect_pascal()?;
                if self.peek() == Some(&Token::Slash) {
                    self.advance();
                    match self.peek() {
                        Some(Token::PascalIdent(_)) => { let v = self.expect_pascal()?; Ok(self.arena.alloc_expr(Expr::PathVariant { typ: n, variant: v, span: s })) }
                        Some(Token::CamelIdent(_)) => {
                            let m = self.expect_camel()?;
                            self.expect(&Token::LParen)?;
                            let mut args = Vec::new();
                            while self.peek() != Some(&Token::RParen) { args.push(self.parse_expr()?); }
                            self.expect(&Token::RParen)?;
                            Ok(self.arena.alloc_expr(Expr::PathMethod { typ: n, method: m, args, span: s }))
                        }
                        other => Err(format!("expected variant/method after /, got {:?}", other)),
                    }
                } else { Ok(self.arena.alloc_expr(Expr::InstanceRef { name: n, span: s })) }
            }
            Some(Token::Integer(_)) => { match self.advance().cloned() { Some(Token::Integer(v)) => Ok(self.arena.alloc_expr(Expr::IntLit { value: v, span: s })), _ => unreachable!() } }
            Some(Token::Float(_)) => { match self.advance().cloned() { Some(Token::Float(v)) => Ok(self.arena.alloc_expr(Expr::FloatLit { value: v.parse().unwrap_or(0.0), span: s })), _ => unreachable!() } }
            Some(Token::StringLit(_)) => { match self.advance().cloned() { Some(Token::StringLit(v)) => Ok(self.arena.alloc_expr(Expr::StringLit { value: v, span: s })), _ => unreachable!() } }
            Some(Token::LBracket) => { let b = self.parse_block()?; Ok(self.arena.alloc_expr(Expr::InlineEval(b))) }
            Some(Token::LParenPipe) => { let m = self.parse_match_expr()?; Ok(self.arena.alloc_expr(Expr::Match(m))) }
            Some(Token::LBracketPipe) => { let b = self.parse_loop_block()?; Ok(self.arena.alloc_expr(Expr::Loop(b))) }
            Some(Token::LBracePipe) => { let (src, body) = self.parse_iteration()?; Ok(self.arena.alloc_expr(Expr::Iteration { source: src, body })) }
            Some(Token::LBrace) => {
                self.expect(&Token::LBrace)?;
                let typ = self.expect_pascal()?;
                let mut refs = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    if self.at_end() { return Err("unexpected EOF in struct construct".into()); }
                    self.expect(&Token::LParen)?;
                    let name = self.expect_pascal()?;
                    let value = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    refs.push(self.arena.alloc_field_init(FieldInit { name, value }));
                }
                self.expect(&Token::RBrace)?;
                Ok(self.arena.alloc_expr(Expr::StructConstruct { typ, fields: refs, span: s }))
            }
            other => Err(format!("expected expression, got {:?}", other)),
        }
    }

    fn parse_match_expr(&mut self) -> Result<MatchRef, String> {
        self.expect(&Token::LParenPipe)?;
        let mut target = None;
        let mut arms = Vec::new();
        if self.peek() != Some(&Token::LParen) && self.peek() != Some(&Token::RPipeParen) {
            target = Some(self.parse_expr()?);
        }
        while self.peek() != Some(&Token::RPipeParen) {
            if self.at_end() { return Err("unexpected EOF in match".into()); }
            let before = self.pos;
            self.expect(&Token::LParen)?;
            let mut pats = Vec::new();
            pats.push(self.parse_pattern()?);
            while self.peek() == Some(&Token::Pipe) { self.advance(); pats.push(self.parse_pattern()?); }
            self.expect(&Token::RParen)?;
            let result = self.parse_expr()?;
            arms.push(self.arena.alloc_match_arm(MatchArm { patterns: pats, result }));
            if self.pos <= before { return Err("stuck in match".into()); }
        }
        self.expect(&Token::RPipeParen)?;
        Ok(self.arena.alloc_match(MatchExpr { target, arms }))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        match self.peek() {
            Some(Token::PascalIdent(_)) => {
                let n = self.expect_pascal()?;
                if self.peek() == Some(&Token::At) { self.advance(); let b = self.expect_pascal()?; Ok(Pattern::VariantBind { variant: n, binding: b }) }
                else { Ok(Pattern::Variant(n)) }
            }
            Some(Token::StringLit(_)) => { match self.advance().cloned() { Some(Token::StringLit(v)) => Ok(Pattern::StringLit(v)), _ => unreachable!() } }
            other => Err(format!("expected pattern, got {:?}", other)),
        }
    }

    fn parse_loop_block(&mut self) -> Result<BlockRef, String> {
        self.expect(&Token::LBracketPipe)?;
        let mut srefs = Vec::new();
        while self.peek() != Some(&Token::RPipeBracket) {
            if self.at_end() { return Err("unexpected EOF in loop".into()); }
            let before = self.pos;
            let s = self.parse_statement()?; srefs.push(self.arena.alloc_stmt(s));
            if self.pos <= before { return Err("stuck in loop".into()); }
        }
        self.expect(&Token::RPipeBracket)?;
        Ok(self.arena.alloc_block(Block { stmts: srefs, tail: None }))
    }

    fn parse_iteration(&mut self) -> Result<(ExprRef, BlockRef), String> {
        self.expect(&Token::LBracePipe)?;
        let source = self.parse_expr()?;
        let body = self.parse_block()?;
        self.expect(&Token::RPipeBrace)?;
        Ok((source, body))
    }

    fn parse_const(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBracePipe)?;
        let name = self.expect_pascal()?;
        let typ = self.parse_type_expr()?;
        let value = match self.peek() {
            Some(Token::Integer(_)) => match self.advance().cloned() { Some(Token::Integer(v)) => LiteralValue::Int(v), _ => unreachable!() },
            Some(Token::Float(_)) => match self.advance().cloned() { Some(Token::Float(v)) => LiteralValue::Float(v.parse().unwrap_or(0.0)), _ => unreachable!() },
            Some(Token::StringLit(_)) => match self.advance().cloned() { Some(Token::StringLit(v)) => LiteralValue::Str(v), _ => unreachable!() },
            other => return Err(format!("expected literal, got {:?}", other)),
        };
        self.expect(&Token::RPipeBrace)?;
        Ok(RootChild::Const(ConstDef { name, typ, value, span: self.span_from(start) }))
    }

    fn parse_ffi(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParenPipe)?;
        let library = self.expect_pascal()?;
        let mut fns = Vec::new();
        while self.peek() != Some(&Token::RPipeParen) {
            if self.at_end() { return Err("unexpected EOF in FFI".into()); }
            fns.push(self.parse_ffi_function()?);
        }
        self.expect(&Token::RPipeParen)?;
        Ok(RootChild::Ffi(FfiDef { library, functions: fns, span: self.span_from(start) }))
    }

    fn parse_ffi_function(&mut self) -> Result<FfiFunction, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut prefs = Vec::new();
        let mut ret = None;
        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in FFI fn".into()); }
            match self.peek() {
                Some(Token::At) => { let p = self.parse_param()?; prefs.push(self.arena.alloc_param(p)); }
                _ => { ret = Some(self.parse_type_expr()?); }
            }
        }
        self.expect(&Token::RParen)?;
        Ok(FfiFunction { name, params: prefs, return_type: ret, span: self.span_from(start) })
    }

    fn parse_process(&mut self) -> Result<RootChild, String> {
        self.expect(&Token::LBracketPipe)?;
        let mut srefs = Vec::new();
        let mut tail = None;
        while self.peek() != Some(&Token::RPipeBracket) {
            if self.at_end() { return Err("unexpected EOF in process".into()); }
            let before = self.pos;
            let stmt = self.parse_statement()?;
            if self.peek() == Some(&Token::RPipeBracket) {
                if let Statement::Expr(e) = stmt { tail = Some(e); break; }
                else { srefs.push(self.arena.alloc_stmt(stmt)); }
            } else { srefs.push(self.arena.alloc_stmt(stmt)); }
            if self.pos <= before { return Err("stuck in process".into()); }
        }
        self.expect(&Token::RPipeBracket)?;
        Ok(RootChild::Process(self.arena.alloc_block(Block { stmts: srefs, tail })))
    }
}

pub fn parse_source(source: &str) -> Result<AskiProgram, String> {
    let tokens = lex(source).map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", "))?;
    let mut parser = Parser::new(&tokens);
    parser.parse_program()
}
