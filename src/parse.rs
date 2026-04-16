/// askic parser — recursive descent following synth rules.
///
/// Reads .aski source into populated domain trees.
/// Hand-written for the bootstrap. The self-hosted version
/// will use data-driven parsing from dialect structures.

use crate::lexer::{Token, Spanned, lex};
use crate::domain::*;

pub struct Parser<'a> {
    tokens: &'a [Spanned],
    pos: usize,
    source: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Spanned], source: &'a str) -> Self {
        Parser { tokens, pos: 0, source }
    }

    pub fn parse_program(&mut self) -> Result<AskiProgram, String> {
        let mut children = Vec::new();

        // first () is module declaration
        let module = self.parse_module_decl()?;
        children.push(RootChild::Module(module));

        // remaining root-level constructs
        while !self.at_end() {
            let child = self.parse_root_child()?;
            children.push(child);
        }

        Ok(AskiProgram { children })
    }

    // ── Token access ────────────────────────────────────────

    fn peek(&self) -> Option<&Token> {
        let mut i = self.pos;
        while i < self.tokens.len() {
            if !matches!(self.tokens[i].token, Token::Newline) {
                return Some(&self.tokens[i].token);
            }
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
        while self.pos < self.tokens.len() && matches!(self.tokens[self.pos].token, Token::Newline) {
            self.pos += 1;
        }
    }

    fn at_end(&self) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() {
            if !matches!(self.tokens[i].token, Token::Newline) {
                return false;
            }
            i += 1;
        }
        true
    }

    fn span_here(&self) -> Span {
        if self.pos < self.tokens.len() {
            let s = &self.tokens[self.pos].span;
            Span { start: s.start as u32, end: s.end as u32 }
        } else {
            Span { start: 0, end: 0 }
        }
    }

    fn span_from(&self, start: u32) -> Span {
        let end = if self.pos > 0 {
            self.tokens[self.pos - 1].span.end as u32
        } else {
            start
        };
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
        let tok = self.advance().cloned();
        match tok {
            Some(Token::PascalIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected PascalCase, got {:?} at pos {}", other, pos)),
        }
    }

    fn expect_camel(&mut self) -> Result<Name, String> {
        let pos = self.pos;
        let tok = self.advance().cloned();
        match tok {
            Some(Token::CamelIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected camelCase, got {:?} at pos {}", other, pos)),
        }
    }

    fn expect_name(&mut self) -> Result<Name, String> {
        let pos = self.pos;
        let tok = self.advance().cloned();
        match tok {
            Some(Token::PascalIdent(s)) | Some(Token::CamelIdent(s)) => Ok(Name::new(&s)),
            other => Err(format!("expected name, got {:?} at pos {}", other, pos)),
        }
    }

    // ── Root level (Root.synth) ─────────────────────────────

    fn parse_module_decl(&mut self) -> Result<ModuleDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_pascal()?;
        let mut exports = Vec::new();
        let mut imports = Vec::new();

        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in module".into()); }
            match self.peek() {
                // import block: [Module Name1 Name2]
                Some(Token::LBracket) => {
                    self.expect(&Token::LBracket)?;
                    let source = self.expect_pascal()?;
                    let mut names = Vec::new();
                    while self.peek() != Some(&Token::RBracket) {
                        names.push(self.expect_name()?);
                    }
                    self.expect(&Token::RBracket)?;
                    imports.push(ModuleImport { source, names });
                }
                // export name
                _ => {
                    exports.push(self.expect_name()?);
                }
            }
        }

        self.expect(&Token::RParen)?;
        Ok(ModuleDef { name, exports, imports, span: self.span_from(start) })
    }

    fn parse_root_child(&mut self) -> Result<RootChild, String> {
        let before = self.pos;
        let result = match self.peek() {
            Some(Token::LParen) => self.parse_root_paren(),
            Some(Token::LBracket) => self.parse_trait_impl(),
            Some(Token::LBrace) => {
                let s = self.parse_struct_def()?;
                Ok(RootChild::Struct(s))
            }
            Some(Token::LBracePipe) => self.parse_const(),
            Some(Token::LParenPipe) => self.parse_ffi(),
            Some(Token::LBracketPipe) => self.parse_process(),
            Some(Token::PascalIdent(_)) => {
                let n = self.parse_newtype_def()?;
                Ok(RootChild::Newtype(n))
            }
            other => Err(format!("expected root construct, got {:?}", other)),
        };
        if self.pos <= before && result.is_ok() {
            return Err(format!("parser stuck at root position {}", self.pos));
        }
        result
    }

    fn parse_root_paren(&mut self) -> Result<RootChild, String> {
        // () at root: enum (PascalCase head) or trait decl (camelCase head)
        self.expect(&Token::LParen)?;
        match self.peek() {
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                let start = self.span_here().start;
                let mut variants = Vec::new();
                while self.peek() != Some(&Token::RParen) {
                    if self.at_end() { return Err("unexpected EOF in enum".into()); }
                    let before = self.pos;
                    variants.push(self.parse_enum_variant()?);
                    if self.pos <= before { return Err("stuck in enum".into()); }
                }
                self.expect(&Token::RParen)?;
                Ok(RootChild::Enum(EnumDef {
                    name,
                    children: variants,
                    span: self.span_from(start),
                }))
            }
            Some(Token::CamelIdent(_)) => {
                let name = self.expect_camel()?;
                let start = self.span_here().start;
                // trait decl: (traitName [signatures])
                self.expect(&Token::LBracket)?;
                let mut sigs = Vec::new();
                while self.peek() != Some(&Token::RBracket) {
                    if self.at_end() { return Err("unexpected EOF in trait decl".into()); }
                    sigs.push(self.parse_method_sig()?);
                }
                self.expect(&Token::RBracket)?;
                self.expect(&Token::RParen)?;
                Ok(RootChild::TraitDecl(TraitDeclDef {
                    name,
                    signatures: sigs,
                    span: self.span_from(start),
                }))
            }
            other => Err(format!("expected PascalCase or camelCase after (, got {:?}", other)),
        }
    }

    // ── Enum variants (Enum.synth) ──────────────────────────

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
                Ok(EnumChild::StructVariant {
                    name: s.name.clone(),
                    fields: s.children.into_iter().map(|c| match c {
                        StructChild::TypedField { name, typ, span } => StructField { name, typ, span },
                        StructChild::SelfTypedField { name, span } => StructField {
                            typ: TypeExpr::Simple(name.clone()),
                            name, span,
                        },
                        _ => panic!("nested definition inside struct variant"),
                    }).collect(),
                    span: s.span,
                })
            }
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                Ok(EnumChild::Variant { name, span: self.span_from(start) })
            }
            other => Err(format!("expected enum variant, got {:?}", other)),
        }
    }

    // ── Struct (Struct.synth) ───────────────────────────────

    fn parse_struct_def(&mut self) -> Result<StructDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBrace)?;
        let name = self.expect_pascal()?;
        let mut children = Vec::new();

        while self.peek() != Some(&Token::RBrace) {
            if self.at_end() { return Err("unexpected EOF in struct".into()); }
            let before = self.pos;
            children.push(self.parse_struct_child()?);
            if self.pos <= before { return Err("stuck in struct".into()); }
        }

        self.expect(&Token::RBrace)?;
        Ok(StructDef { name, children, span: self.span_from(start) })
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

    // ── Newtype (bare at root) ──────────────────────────────

    fn parse_newtype_def(&mut self) -> Result<NewtypeDef, String> {
        let start = self.span_here().start;
        let name = self.expect_pascal()?;
        let wraps = self.parse_type_expr()?;
        Ok(NewtypeDef { name, wraps, span: self.span_from(start) })
    }

    // ── Type expressions (Type.synth) ───────────────────────

    fn parse_type_expr(&mut self) -> Result<TypeExpr, String> {
        match self.peek() {
            Some(Token::LBracket) => {
                self.expect(&Token::LBracket)?;
                let constructor = self.expect_pascal()?;
                let mut args = Vec::new();
                while self.peek() != Some(&Token::RBracket) {
                    if self.at_end() { return Err("unexpected EOF in type application".into()); }
                    let before = self.pos;
                    args.push(self.parse_type_expr()?);
                    if self.pos <= before { return Err("stuck in type application".into()); }
                }
                self.expect(&Token::RBracket)?;
                Ok(TypeExpr::Application { constructor, args })
            }
            Some(Token::Dollar) => {
                self.advance(); // consume $
                let first = self.expect_pascal()?;
                let mut bounds = vec![first];
                while self.peek() == Some(&Token::Ampersand) {
                    self.advance(); // consume &
                    bounds.push(self.expect_pascal()?);
                }
                if bounds.len() == 1 {
                    Ok(TypeExpr::Param(bounds.remove(0)))
                } else {
                    Ok(TypeExpr::BoundedParam { bounds })
                }
            }
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                Ok(TypeExpr::Simple(name))
            }
            other => Err(format!("expected type expression, got {:?}", other)),
        }
    }

    // ── Trait impl (TraitImpl.synth) ────────────────────────

    fn parse_trait_impl(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBracket)?;
        let trait_name = self.expect_camel()?;
        let type_name = self.expect_pascal()?;

        self.expect(&Token::LBracket)?;
        let mut methods = Vec::new();
        while self.peek() != Some(&Token::RBracket) {
            if self.at_end() { return Err("unexpected EOF in trait impl".into()); }
            methods.push(self.parse_method_def()?);
        }
        self.expect(&Token::RBracket)?;
        self.expect(&Token::RBracket)?;

        Ok(RootChild::TraitImpl(TraitImplDef {
            trait_name, type_name, methods,
            span: self.span_from(start),
        }))
    }

    // ── Method signature (Signature.synth) ──────────────────

    fn parse_method_sig(&mut self) -> Result<MethodSig, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut params = Vec::new();
        let mut return_type = None;

        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in signature".into()); }
            match self.peek() {
                Some(Token::Colon) | Some(Token::Tilde) | Some(Token::At) => {
                    params.push(self.parse_param()?);
                }
                _ => {
                    return_type = Some(self.parse_type_expr()?);
                    break;
                }
            }
        }

        self.expect(&Token::RParen)?;
        Ok(MethodSig { name, params, return_type, span: self.span_from(start) })
    }

    // ── Method definition (Method.synth) ────────────────────

    fn parse_method_def(&mut self) -> Result<MethodDef, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut params = Vec::new();
        let mut return_type = None;

        // parse params until we hit a body delimiter or return type
        loop {
            match self.peek() {
                Some(Token::Colon) | Some(Token::Tilde) | Some(Token::At) => {
                    params.push(self.parse_param()?);
                }
                Some(Token::LBracket) | Some(Token::LParenPipe) | Some(Token::LBracketPipe)
                | Some(Token::LBracePipe) | Some(Token::LBrace) | Some(Token::RParen) => break,
                _ => {
                    // could be return type
                    return_type = Some(self.parse_type_expr()?);
                }
            }
        }

        // parse body
        let body = if self.peek() == Some(&Token::RParen) {
            // no body — declaration only
            MethodBody::Block(Block { statements: vec![], tail: None })
        } else {
            self.parse_method_body()?
        };

        self.expect(&Token::RParen)?;
        Ok(MethodDef { name, params, return_type, body, span: self.span_from(start) })
    }

    // ── Params (Param.synth) ────────────────────────────────

    fn parse_param(&mut self) -> Result<Param, String> {
        match self.peek() {
            Some(Token::Colon) => {
                self.advance(); // :
                self.expect(&Token::At)?;
                match self.peek() {
                    Some(Token::PascalIdent(s)) if s == "Self" => {
                        self.advance();
                        Ok(Param::BorrowSelf)
                    }
                    _ => {
                        let name = self.expect_pascal()?;
                        let typ = self.parse_type_expr()?;
                        Ok(Param::Named { name, typ })
                    }
                }
            }
            Some(Token::Tilde) => {
                self.advance(); // ~
                self.expect(&Token::At)?;
                match self.peek() {
                    Some(Token::PascalIdent(s)) if s == "Self" => {
                        self.advance();
                        Ok(Param::MutBorrowSelf)
                    }
                    _ => {
                        let name = self.expect_pascal()?;
                        let typ = self.parse_type_expr()?;
                        Ok(Param::Named { name, typ })
                    }
                }
            }
            Some(Token::At) => {
                self.advance(); // @
                match self.peek() {
                    Some(Token::PascalIdent(s)) if s == "Self" => {
                        self.advance();
                        Ok(Param::OwnedSelf)
                    }
                    _ => {
                        let name = self.expect_pascal()?;
                        if matches!(self.peek(), Some(Token::PascalIdent(_)) | Some(Token::LBracket) | Some(Token::Dollar)) {
                            let typ = self.parse_type_expr()?;
                            Ok(Param::Named { name, typ })
                        } else {
                            Ok(Param::Bare { name })
                        }
                    }
                }
            }
            other => Err(format!("expected param (: ~ @), got {:?}", other)),
        }
    }

    // ── Method body (Method.synth) ──────────────────────────

    fn parse_method_body(&mut self) -> Result<MethodBody, String> {
        match self.peek() {
            Some(Token::LBracket) => {
                let block = self.parse_block()?;
                Ok(MethodBody::Block(block))
            }
            Some(Token::LParenPipe) => {
                let m = self.parse_match_expr()?;
                Ok(MethodBody::Match(m))
            }
            Some(Token::LBracketPipe) => {
                let l = self.parse_loop()?;
                Ok(MethodBody::Loop(l))
            }
            Some(Token::LBracePipe) => {
                let i = self.parse_iteration()?;
                Ok(MethodBody::Iteration(i))
            }
            Some(Token::LBrace) => {
                let start = self.span_here().start;
                self.expect(&Token::LBrace)?;
                let typ = self.expect_pascal()?;
                let mut fields = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    if self.at_end() { return Err("unexpected EOF in struct construct".into()); }
                    self.expect(&Token::LParen)?;
                    let name = self.expect_pascal()?;
                    let value = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    fields.push(FieldInit { name, value: Box::new(value) });
                }
                self.expect(&Token::RBrace)?;
                Ok(MethodBody::StructConstruct { typ, fields, span: self.span_from(start) })
            }
            other => Err(format!("expected method body, got {:?}", other)),
        }
    }

    // ── Block (Body.synth) ──────────────────────────────────

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&Token::LBracket)?;
        let mut statements = Vec::new();
        let mut tail = None;

        while self.peek() != Some(&Token::RBracket) {
            if self.at_end() { return Err("unexpected EOF in block".into()); }
            let before = self.pos;
            let stmt = self.parse_statement()?;

            // last item might be tail expression
            if self.peek() == Some(&Token::RBracket) {
                if let Statement::Expr(expr) = stmt {
                    tail = Some(expr);
                    break;
                } else {
                    statements.push(stmt);
                }
            } else {
                statements.push(stmt);
            }

            if self.pos <= before { return Err("stuck in block".into()); }
        }

        self.expect(&Token::RBracket)?;
        Ok(Block { statements, tail })
    }

    // ── Statement (Statement.synth) ─────────────────────────

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek() {
            Some(Token::Caret) => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Statement::EarlyReturn(Box::new(expr)))
            }
            Some(Token::LBracketPipe) => {
                let l = self.parse_loop()?;
                Ok(Statement::Loop(l))
            }
            Some(Token::LBracePipe) => {
                let i = self.parse_iteration()?;
                Ok(Statement::Iteration(i))
            }
            Some(Token::LParen) => {
                // local type declaration: (Name Type)
                let start = self.span_here().start;
                self.expect(&Token::LParen)?;
                let name = self.expect_pascal()?;
                let typ = self.parse_type_expr()?;
                self.expect(&Token::RParen)?;
                Ok(Statement::LocalTypeDecl { name, typ, span: self.span_from(start) })
            }
            Some(Token::Tilde) => {
                self.advance(); // ~
                self.expect(&Token::At)?;
                let name = self.expect_pascal()?;
                // mutation: ~@Name.method(args)
                if self.peek() == Some(&Token::Dot) {
                    self.advance();
                    let method = self.expect_camel()?;
                    self.expect(&Token::LParen)?;
                    let mut args = Vec::new();
                    while self.peek() != Some(&Token::RParen) {
                        args.push(self.parse_expr()?);
                    }
                    self.expect(&Token::RParen)?;
                    let span = self.span_here();
                    Ok(Statement::Mutation(Mutation { name, method, args, span }))
                } else {
                    // mutable instance: ~@Name Expr
                    let value = self.parse_expr()?;
                    let span = self.span_here();
                    Ok(Statement::Instance(Instance {
                        name, type_annotation: None,
                        value: Box::new(value), span,
                    }))
                }
            }
            Some(Token::At) => {
                // peek ahead: @Name followed by . or ? is an expression, not instance
                let saved = self.pos;
                self.advance(); // @
                let name = self.expect_pascal()?;
                match self.peek() {
                    Some(Token::Dot) | Some(Token::Question)
                    | Some(Token::Plus) | Some(Token::Minus)
                    | Some(Token::Star) | Some(Token::Percent)
                    | Some(Token::DoubleEquals) | Some(Token::NotEqual)
                    | Some(Token::LessThan) | Some(Token::GreaterThan)
                    | Some(Token::LessThanOrEqual) | Some(Token::GreaterThanOrEqual)
                    | Some(Token::LogicalAnd) | Some(Token::LogicalOr) => {
                        // it's an expression starting with @Name
                        self.pos = saved;
                        let expr = self.parse_expr()?;
                        return Ok(Statement::Expr(Box::new(expr)));
                    }
                    _ => {}
                }
                // optional type annotation: @Name (Type) Expr
                let type_annotation = if self.peek() == Some(&Token::LParen) {
                    // peek further — is this (Type) or (Expr)?
                    // for now, try type annotation
                    let saved = self.pos;
                    self.expect(&Token::LParen)?;
                    match self.parse_type_expr() {
                        Ok(typ) => {
                            if self.peek() == Some(&Token::RParen) {
                                self.expect(&Token::RParen)?;
                                Some(typ)
                            } else {
                                self.pos = saved; // backtrack
                                None
                            }
                        }
                        Err(_) => {
                            self.pos = saved; // backtrack
                            None
                        }
                    }
                } else {
                    None
                };
                let value = self.parse_expr()?;
                let span = self.span_here();
                Ok(Statement::Instance(Instance {
                    name, type_annotation, value: Box::new(value), span,
                }))
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Statement::Expr(Box::new(expr)))
            }
        }
    }

    // ── Expressions (ExprOr..ExprAtom) ──────────────────────

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_expr_or()
    }

    fn parse_expr_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_expr_and()?;
        while self.peek() == Some(&Token::LogicalOr) {
            let span = self.span_here();
            self.advance();
            let right = self.parse_expr_and()?;
            left = Expr::BinOr { left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_expr_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_expr_compare()?;
        while self.peek() == Some(&Token::LogicalAnd) {
            let span = self.span_here();
            self.advance();
            let right = self.parse_expr_compare()?;
            left = Expr::BinAnd { left: Box::new(left), right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_expr_compare(&mut self) -> Result<Expr, String> {
        let left = self.parse_expr_add()?;
        let span = self.span_here();
        match self.peek() {
            Some(Token::DoubleEquals) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinEq { left: Box::new(left), right: Box::new(r), span }) }
            Some(Token::NotEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinNotEq { left: Box::new(left), right: Box::new(r), span }) }
            Some(Token::LessThan) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinLt { left: Box::new(left), right: Box::new(r), span }) }
            Some(Token::GreaterThan) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinGt { left: Box::new(left), right: Box::new(r), span }) }
            Some(Token::LessThanOrEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinLtEq { left: Box::new(left), right: Box::new(r), span }) }
            Some(Token::GreaterThanOrEqual) => { self.advance(); let r = self.parse_expr_add()?; Ok(Expr::BinGtEq { left: Box::new(left), right: Box::new(r), span }) }
            _ => Ok(left),
        }
    }

    fn parse_expr_add(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_expr_mul()?;
        loop {
            let span = self.span_here();
            match self.peek() {
                Some(Token::Plus) => { self.advance(); let r = self.parse_expr_mul()?; left = Expr::BinAdd { left: Box::new(left), right: Box::new(r), span }; }
                Some(Token::Minus) => { self.advance(); let r = self.parse_expr_mul()?; left = Expr::BinSub { left: Box::new(left), right: Box::new(r), span }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_expr_mul(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_expr_postfix()?;
        loop {
            let span = self.span_here();
            match self.peek() {
                Some(Token::Star) => { self.advance(); let r = self.parse_expr_postfix()?; left = Expr::BinMul { left: Box::new(left), right: Box::new(r), span }; }
                Some(Token::Percent) => { self.advance(); let r = self.parse_expr_postfix()?; left = Expr::BinMod { left: Box::new(left), right: Box::new(r), span }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_expr_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_expr_atom()?;
        loop {
            match self.peek() {
                Some(Token::Dot) => {
                    let span = self.span_here();
                    self.advance();
                    match self.peek() {
                        Some(Token::PascalIdent(_)) => {
                            let field = self.expect_pascal()?;
                            expr = Expr::FieldAccess { object: Box::new(expr), field, span };
                        }
                        Some(Token::CamelIdent(_)) => {
                            let method = self.expect_camel()?;
                            let mut args = Vec::new();
                            if self.peek() == Some(&Token::LParen) {
                                self.advance(); // (
                                while self.peek() != Some(&Token::RParen) {
                                    args.push(self.parse_expr()?);
                                }
                                self.expect(&Token::RParen)?;
                            }
                            expr = Expr::MethodCall { object: Box::new(expr), method, args, span };
                        }
                        other => return Err(format!("expected field or method after ., got {:?}", other)),
                    }
                }
                Some(Token::Question) => {
                    let span = self.span_here();
                    self.advance();
                    expr = Expr::TryUnwrap { inner: Box::new(expr), span };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_expr_atom(&mut self) -> Result<Expr, String> {
        let span = self.span_here();
        match self.peek() {
            // @Name — instance ref
            Some(Token::At) => {
                self.advance();
                let name = self.expect_pascal()?;
                Ok(Expr::InstanceRef { name, span })
            }
            // PascalCase — could be variant or Type/path
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                if self.peek() == Some(&Token::Slash) {
                    self.advance(); // /
                    match self.peek() {
                        Some(Token::PascalIdent(_)) => {
                            let variant = self.expect_pascal()?;
                            Ok(Expr::PathVariant { typ: name, variant, span })
                        }
                        Some(Token::CamelIdent(_)) => {
                            let method = self.expect_camel()?;
                            self.expect(&Token::LParen)?;
                            let mut args = Vec::new();
                            while self.peek() != Some(&Token::RParen) {
                                args.push(self.parse_expr()?);
                            }
                            self.expect(&Token::RParen)?;
                            Ok(Expr::PathMethod { typ: name, method, args, span })
                        }
                        other => Err(format!("expected variant or method after /, got {:?}", other)),
                    }
                } else {
                    Ok(Expr::InstanceRef { name, span })
                }
            }
            // integer literal
            Some(Token::Integer(_)) => {
                match self.advance().cloned() {
                    Some(Token::Integer(v)) => Ok(Expr::IntLit { value: v, span }),
                    _ => unreachable!(),
                }
            }
            // float literal
            Some(Token::Float(_)) => {
                match self.advance().cloned() {
                    Some(Token::Float(v)) => Ok(Expr::FloatLit { value: v.parse().unwrap_or(0.0), span }),
                    _ => unreachable!(),
                }
            }
            // string literal
            Some(Token::StringLit(_)) => {
                match self.advance().cloned() {
                    Some(Token::StringLit(v)) => Ok(Expr::StringLit { value: v, span }),
                    _ => unreachable!(),
                }
            }
            // [body] — inline eval
            Some(Token::LBracket) => {
                let block = self.parse_block()?;
                Ok(Expr::InlineEval(block))
            }
            // (|match|) — match expression
            Some(Token::LParenPipe) => {
                let m = self.parse_match_expr()?;
                Ok(Expr::Match(m))
            }
            // [|loop|] — loop expression
            Some(Token::LBracketPipe) => {
                let l = self.parse_loop()?;
                Ok(Expr::Loop(l))
            }
            // {|iteration|} — iteration expression
            Some(Token::LBracePipe) => {
                let i = self.parse_iteration()?;
                Ok(Expr::Iteration(i))
            }
            // {struct construct} — struct construction
            Some(Token::LBrace) => {
                self.expect(&Token::LBrace)?;
                let typ = self.expect_pascal()?;
                let mut fields = Vec::new();
                while self.peek() != Some(&Token::RBrace) {
                    if self.at_end() { return Err("unexpected EOF in struct construct".into()); }
                    self.expect(&Token::LParen)?;
                    let name = self.expect_pascal()?;
                    let value = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    fields.push(FieldInit { name, value: Box::new(value) });
                }
                self.expect(&Token::RBrace)?;
                Ok(Expr::StructConstruct { typ, fields, span })
            }
            other => Err(format!("expected expression, got {:?}", other)),
        }
    }

    // ── Match (Match.synth) ─────────────────────────────────

    fn parse_match_expr(&mut self) -> Result<MatchExpr, String> {
        self.expect(&Token::LParenPipe)?;
        let mut target = None;
        let mut arms = Vec::new();

        // optional target expression before first arm
        if self.peek() != Some(&Token::LParen) && self.peek() != Some(&Token::RPipeParen) {
            target = Some(Box::new(self.parse_expr()?));
        }

        while self.peek() != Some(&Token::RPipeParen) {
            if self.at_end() { return Err("unexpected EOF in match".into()); }
            let before = self.pos;

            // parse pattern(s)
            self.expect(&Token::LParen)?;
            let mut patterns = Vec::new();
            let first = self.parse_pattern()?;
            patterns.push(first);
            while self.peek() == Some(&Token::Pipe) {
                self.advance(); // |
                patterns.push(self.parse_pattern()?);
            }
            self.expect(&Token::RParen)?;

            // parse result expression
            let result = self.parse_expr()?;
            arms.push(MatchArm { patterns, result: Box::new(result) });

            if self.pos <= before { return Err("stuck in match".into()); }
        }

        self.expect(&Token::RPipeParen)?;
        Ok(MatchExpr { target, arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        match self.peek() {
            Some(Token::PascalIdent(_)) => {
                let name = self.expect_pascal()?;
                if self.peek() == Some(&Token::At) {
                    self.advance(); // @
                    let binding = self.expect_pascal()?;
                    Ok(Pattern::VariantBind { variant: name, binding })
                } else {
                    Ok(Pattern::Variant(name))
                }
            }
            Some(Token::StringLit(_)) => {
                match self.advance().cloned() {
                    Some(Token::StringLit(s)) => Ok(Pattern::StringLit(s)),
                    _ => unreachable!(),
                }
            }
            other => Err(format!("expected pattern, got {:?}", other)),
        }
    }

    // ── Loop (Loop.synth) ───────────────────────────────────

    fn parse_loop(&mut self) -> Result<Loop, String> {
        self.expect(&Token::LBracketPipe)?;
        let mut condition = None;
        let mut body = Vec::new();

        // try parsing condition expression, then statements
        // if the first thing IS a statement, no condition
        // heuristic: if first thing starts with @ ~ ^ [| {| ( or is a literal,
        // AND there are more items, treat as statement
        // otherwise it's a condition

        // for now: parse everything as statements
        // TODO: proper condition detection
        while self.peek() != Some(&Token::RPipeBracket) {
            if self.at_end() { return Err("unexpected EOF in loop".into()); }
            let before = self.pos;
            body.push(self.parse_statement()?);
            if self.pos <= before { return Err("stuck in loop".into()); }
        }
        self.expect(&Token::RPipeBracket)?;
        let _ = condition; // TODO
        Ok(Loop { condition, body })
    }

    // ── Iteration ───────────────────────────────────────────

    fn parse_iteration(&mut self) -> Result<Iteration, String> {
        self.expect(&Token::LBracePipe)?;
        let source = self.parse_expr()?;
        let block = self.parse_block()?;
        self.expect(&Token::RPipeBrace)?;
        Ok(Iteration { source: Box::new(source), body: block })
    }

    // ── Const ───────────────────────────────────────────────

    fn parse_const(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LBracePipe)?;
        let name = self.expect_pascal()?;
        let typ = self.parse_type_expr()?;
        let value = match self.peek() {
            Some(Token::Integer(_)) => {
                match self.advance().cloned() {
                    Some(Token::Integer(v)) => LiteralValue::Int(v),
                    _ => unreachable!(),
                }
            }
            Some(Token::Float(_)) => {
                match self.advance().cloned() {
                    Some(Token::Float(v)) => LiteralValue::Float(v.parse().unwrap_or(0.0)),
                    _ => unreachable!(),
                }
            }
            Some(Token::StringLit(_)) => {
                match self.advance().cloned() {
                    Some(Token::StringLit(v)) => LiteralValue::Str(v),
                    _ => unreachable!(),
                }
            }
            other => return Err(format!("expected literal value in const, got {:?}", other)),
        };
        self.expect(&Token::RPipeBrace)?;
        Ok(RootChild::Const(ConstDef { name, typ, value, span: self.span_from(start) }))
    }

    // ── FFI ─────────────────────────────────────────────────

    fn parse_ffi(&mut self) -> Result<RootChild, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParenPipe)?;
        let library = self.expect_pascal()?;
        let mut functions = Vec::new();

        while self.peek() != Some(&Token::RPipeParen) {
            if self.at_end() { return Err("unexpected EOF in FFI".into()); }
            functions.push(self.parse_ffi_function()?);
        }
        self.expect(&Token::RPipeParen)?;
        Ok(RootChild::Ffi(FfiDef { library, functions, span: self.span_from(start) }))
    }

    fn parse_ffi_function(&mut self) -> Result<FfiFunction, String> {
        let start = self.span_here().start;
        self.expect(&Token::LParen)?;
        let name = self.expect_camel()?;
        let mut params = Vec::new();
        let mut return_type = None;

        while self.peek() != Some(&Token::RParen) {
            if self.at_end() { return Err("unexpected EOF in FFI function".into()); }
            match self.peek() {
                Some(Token::At) => {
                    params.push(self.parse_param()?);
                }
                _ => {
                    return_type = Some(self.parse_type_expr()?);
                }
            }
        }
        self.expect(&Token::RParen)?;
        Ok(FfiFunction { name, params, return_type, span: self.span_from(start) })
    }

    // ── Process ─────────────────────────────────────────────

    fn parse_process(&mut self) -> Result<RootChild, String> {
        self.expect(&Token::LBracketPipe)?;
        let mut statements = Vec::new();
        let mut tail = None;

        while self.peek() != Some(&Token::RPipeBracket) {
            if self.at_end() { return Err("unexpected EOF in process".into()); }
            let before = self.pos;
            let stmt = self.parse_statement()?;
            if self.peek() == Some(&Token::RPipeBracket) {
                if let Statement::Expr(expr) = stmt {
                    tail = Some(expr);
                    break;
                } else {
                    statements.push(stmt);
                }
            } else {
                statements.push(stmt);
            }
            if self.pos <= before { return Err("stuck in process".into()); }
        }

        self.expect(&Token::RPipeBracket)?;
        Ok(RootChild::Process(Block { statements, tail }))
    }
}

/// Parse an .aski source file into an AskiProgram.
pub fn parse_source(source: &str) -> Result<AskiProgram, String> {
    let tokens = lex(source).map_err(|errs| {
        errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ")
    })?;
    let mut parser = Parser::new(&tokens, source);
    parser.parse_program()
}
