/// Machine — dialect state machine engine.
///
/// Walks the ArchivedDialectTree from askicc, matches tokens,
/// constructs typed aski-core output directly. No separate builder.
/// No ParseValue::Seq. No untyped bags.
///
/// The dialect data IS the state machine. The engine reads it
/// and follows it. Every match produces a value whose type is
/// known from the state, not from runtime guessing.

use std::collections::HashMap;
use synth_core::*;
use aski_core::*;
use crate::lexer::{Token, Spanned};
use crate::typed::{Typed, ItemTuple};

// ── Cursor ──────────────────────────────────────────────────

pub struct Cursor<'a> {
    tokens: &'a [Spanned],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(tokens: &'a [Spanned]) -> Self {
        Cursor { tokens, pos: 0 }
    }

    pub fn pos(&self) -> usize { self.pos }
    pub fn restore(&mut self, pos: usize) { self.pos = pos; }

    pub fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub fn peek(&self) -> Option<&Token> {
        // Skip newlines
        let mut p = self.pos;
        while p < self.tokens.len() {
            if !matches!(self.tokens[p].token, Token::Newline) {
                return Some(&self.tokens[p].token);
            }
            p += 1;
        }
        None
    }

    pub fn peek_raw(&self) -> Option<&Spanned> {
        let mut p = self.pos;
        while p < self.tokens.len() {
            if !matches!(self.tokens[p].token, Token::Newline) {
                return Some(&self.tokens[p]);
            }
            p += 1;
        }
        None
    }

    pub fn advance(&mut self) -> Option<&Spanned> {
        // Skip newlines
        while self.pos < self.tokens.len() {
            if matches!(self.tokens[self.pos].token, Token::Newline) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    pub fn span(&self) -> Span {
        if let Some(tok) = self.peek_raw() {
            Span { start: tok.span.start as u32, end: tok.span.end as u32 }
        } else {
            Span { start: 0, end: 0 }
        }
    }

    pub fn last_span(&self) -> Span {
        if self.pos > 0 {
            let tok = &self.tokens[self.pos - 1];
            Span { start: tok.span.start as u32, end: tok.span.end as u32 }
        } else {
            Span { start: 0, end: 0 }
        }
    }

    pub fn expect(&mut self, expected: &Token) -> Result<Span, String> {
        let sp = self.span();
        match self.advance() {
            Some(tok) if tok.token == *expected => Ok(sp),
            Some(tok) => Err(format!("expected {:?}, got {:?}", expected, tok.token)),
            None => Err(format!("expected {:?}, got EOF", expected)),
        }
    }

    pub fn is_adjacent(&self) -> bool {
        if self.pos == 0 || self.pos >= self.tokens.len() {
            return false;
        }
        let prev = &self.tokens[self.pos - 1];
        let curr = &self.tokens[self.pos];
        prev.span.end == curr.span.start
    }
}

// ── Engine ──────────────────────────────────────────────────

pub struct Machine {
    data: &'static [u8],
    dialect_index: HashMap<u8, usize>,
}

impl Machine {
    pub fn new(data: &'static [u8]) -> Self {
        let tree = Self::access_tree(data);
        let mut dialect_index = HashMap::new();
        for (i, dialect) in tree.dialects.iter().enumerate() {
            let disc = Self::dialect_discriminant(&dialect.kind);
            dialect_index.insert(disc, i);
        }
        Machine { data, dialect_index }
    }

    fn access_tree(data: &[u8]) -> &ArchivedDialectTree {
        unsafe { rkyv::access_unchecked::<ArchivedDialectTree>(data) }
    }

    fn tree(&self) -> &ArchivedDialectTree {
        Self::access_tree(self.data)
    }

    fn dialect_discriminant(kind: &ArchivedDialectKind) -> u8 {
        unsafe { *(kind as *const ArchivedDialectKind as *const u8) }
    }

    fn lookup(&self, kind: &ArchivedDialectKind) -> &ArchivedDialect {
        let disc = Self::dialect_discriminant(kind);
        let idx = self.dialect_index.get(&disc)
            .expect("dialect not found");
        &self.tree().dialects[*idx]
    }

    // ── Public API ──────────────────────────────────────────

    pub fn parse(&self, tokens: &[Spanned]) -> Result<ModuleDef, String> {
        let mut cursor = Cursor::new(tokens);
        self.enter_dialect(&ArchivedDialectKind::Root, &mut cursor)?
            .into_module()
    }

    // ── Dialect dispatch ────────────────────────────────────

    fn enter_dialect(
        &self,
        kind: &ArchivedDialectKind,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // Expr pass-through
        if matches!(kind, ArchivedDialectKind::Expr) {
            return self.enter_dialect(&ArchivedDialectKind::ExprOr, cursor);
        }
        // ExprPostfix is left-recursive
        if matches!(kind, ArchivedDialectKind::ExprPostfix) {
            return self.parse_postfix(cursor);
        }

        let dialect = self.lookup(kind);

        // Process each rule
        match kind {
            ArchivedDialectKind::Root => self.parse_root(dialect, cursor),
            ArchivedDialectKind::Module => self.parse_module(dialect, cursor),
            ArchivedDialectKind::Enum => self.parse_enum(dialect, cursor),
            ArchivedDialectKind::Struct => self.parse_struct(dialect, cursor),
            ArchivedDialectKind::Body => self.parse_body(dialect, cursor),
            ArchivedDialectKind::Type_ => self.parse_type(dialect, cursor),
            ArchivedDialectKind::TypeApplication => self.parse_type_application(dialect, cursor),
            ArchivedDialectKind::GenericParam => self.parse_generic_param(dialect, cursor),
            ArchivedDialectKind::Statement => self.parse_statement(dialect, cursor),
            ArchivedDialectKind::Instance => self.parse_instance(dialect, cursor),
            ArchivedDialectKind::Mutation => self.parse_mutation(dialect, cursor),
            ArchivedDialectKind::Param => self.parse_param(dialect, cursor),
            ArchivedDialectKind::Signature => self.parse_signature(dialect, cursor),
            ArchivedDialectKind::Method => self.parse_method(dialect, cursor),
            ArchivedDialectKind::TraitDecl => self.parse_trait_decl(dialect, cursor),
            ArchivedDialectKind::TraitImpl => self.parse_trait_impl(dialect, cursor),
            ArchivedDialectKind::TypeImpl => self.parse_type_impl(dialect, cursor),
            ArchivedDialectKind::Match => self.parse_match(dialect, cursor),
            ArchivedDialectKind::Pattern => self.parse_pattern(dialect, cursor),
            ArchivedDialectKind::Loop => self.parse_loop(dialect, cursor),
            ArchivedDialectKind::Process => self.parse_process(dialect, cursor),
            ArchivedDialectKind::IterationSource => self.parse_iteration_source(dialect, cursor),
            ArchivedDialectKind::StructConstruct => self.parse_struct_construct(dialect, cursor),
            ArchivedDialectKind::Ffi => self.parse_ffi(dialect, cursor),
            ArchivedDialectKind::ExprOr => self.parse_expr_binary(dialect, cursor, BinOp::Or),
            ArchivedDialectKind::ExprAnd => self.parse_expr_binary(dialect, cursor, BinOp::And),
            ArchivedDialectKind::ExprCompare => self.parse_expr_compare(dialect, cursor),
            ArchivedDialectKind::ExprAdd => self.parse_expr_add(dialect, cursor),
            ArchivedDialectKind::ExprMul => self.parse_expr_mul(dialect, cursor),
            ArchivedDialectKind::ExprAtom => self.parse_expr_atom(dialect, cursor),
            _ => Err("unhandled dialect".into()),
        }
    }

    // ── Item matching ───────────────────────────────────────

    /// Match a single item from the dialect data.
    fn match_item(
        &self,
        item: &ArchivedItem,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        match &item.content {
            ArchivedItemContent::Named { label } => self.match_named(label, cursor),
            ArchivedItemContent::DialectRef { target } => self.enter_dialect(target, cursor),
            ArchivedItemContent::Delimited { kind, inner } => {
                let tuple = self.match_delimited(kind, inner, cursor)?;
                Ok(Typed::Group(tuple))
            }
            ArchivedItemContent::Literal { token } => self.match_literal(token, cursor),
            ArchivedItemContent::Keyword { token } => self.match_keyword(token, cursor),
            ArchivedItemContent::Repeat { kind, inner } => {
                self.match_repeat(kind, inner, cursor)
            }
            ArchivedItemContent::LiteralValue => self.match_literal_value(cursor),
        }
    }

    /// Match items sequentially, return as ItemTuple.
    fn match_items_seq(
        &self,
        items: &rkyv::vec::ArchivedVec<ArchivedItem>,
        cursor: &mut Cursor,
    ) -> Result<ItemTuple, String> {
        let mut values = Vec::with_capacity(items.len());
        for item in items.iter() {
            values.push(self.match_item(item, cursor)?);
        }
        Ok(ItemTuple(values))
    }

    /// Match a delimited group: open, inner items, close.
    fn match_delimited(
        &self,
        kind: &ArchivedDelimKind,
        inner: &rkyv::vec::ArchivedVec<ArchivedItem>,
        cursor: &mut Cursor,
    ) -> Result<ItemTuple, String> {
        let open = Self::delim_open_token(kind);
        let close = Self::delim_close_token(kind);
        cursor.expect(&open)?;
        let result = self.match_items_seq(inner, cursor)?;
        cursor.expect(&close)?;
        Ok(result)
    }

    /// Unwrap a delimited item and match its contents.
    fn match_delimited_item(
        &self,
        item: &ArchivedItem,
        cursor: &mut Cursor,
    ) -> Result<ItemTuple, String> {
        match &item.content {
            ArchivedItemContent::Delimited { kind, inner } => {
                self.match_delimited(kind, inner, cursor)
            }
            _ => Err("expected delimited item".into()),
        }
    }

    /// Match a Named label. Uses Label.casing to determine token type.
    fn match_named(
        &self,
        label: &ArchivedLabel,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // Special case: LabelKind::Literal reads a value, not a name
        if matches!(label.kind, ArchivedLabelKind::Literal) {
            return self.match_literal_value(cursor);
        }

        let is_pascal = matches!(label.casing, ArchivedCasing::Pascal);
        let span = cursor.span();

        match cursor.advance() {
            Some(tok) => {
                let name = match &tok.token {
                    Token::PascalIdent(s) if is_pascal => s.clone(),
                    Token::CamelIdent(s) if !is_pascal => s.clone(),
                    other => return Err(format!(
                        "expected {} name, got {:?}",
                        if is_pascal { "PascalCase" } else { "camelCase" },
                        other)),
                };
                if is_pascal {
                    Ok(Typed::PascalName(name, span))
                } else {
                    Ok(Typed::CamelName(name, span))
                }
            }
            None => Err("unexpected end of tokens".into()),
        }
    }

    /// Match a literal token.
    fn match_literal(
        &self,
        token: &ArchivedLiteralToken,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let span = cursor.span();
        let expected = Self::literal_to_token(token);

        // Multi-token literals
        match token {
            ArchivedLiteralToken::BorrowAt => {
                cursor.expect(&Token::Colon)?;
                cursor.expect(&Token::At)?;
                return Ok(Typed::Token(span));
            }
            ArchivedLiteralToken::MutAt => {
                cursor.expect(&Token::Tilde)?;
                cursor.expect(&Token::At)?;
                return Ok(Typed::Token(span));
            }
            _ => {}
        }

        match cursor.advance() {
            Some(tok) if tok.token == expected => Ok(Typed::Token(span)),
            Some(tok) => Err(format!("expected {:?}, got {:?}", expected, tok.token)),
            None => Err(format!("expected {:?}, got EOF", expected)),
        }
    }

    /// Match a keyword (Self, Main).
    fn match_keyword(
        &self,
        token: &ArchivedKeywordToken,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let span = cursor.span();
        let expected = match token {
            ArchivedKeywordToken::Self_ => "Self",
            ArchivedKeywordToken::Main => "Main",
        };
        match cursor.advance() {
            Some(tok) => {
                match &tok.token {
                    Token::PascalIdent(s) if s == expected => Ok(Typed::Keyword(span)),
                    other => Err(format!("expected keyword {}, got {:?}", expected, other)),
                }
            }
            None => Err(format!("expected keyword {}, got EOF", expected)),
        }
    }

    /// Match a literal value (integer, float, string).
    fn match_literal_value(
        &self,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let span = cursor.span();
        match cursor.advance() {
            Some(tok) => {
                let value = match &tok.token {
                    Token::Integer(n) => LiteralValue::Int(*n),
                    Token::Float(f) => LiteralValue::Float(
                        f.parse::<f64>().map_err(|e| format!("invalid float: {}", e))?
                    ),
                    Token::StringLit(s) => LiteralValue::Str(s.clone()),
                    other => return Err(format!("expected literal value, got {:?}", other)),
                };
                Ok(Typed::Literal(value, span))
            }
            None => Err("expected literal value, got EOF".into()),
        }
    }

    /// Match a repeat (*, +, ?). Produces a typed collection
    /// based on the inner item's type.
    fn match_repeat(
        &self,
        cardinality: &ArchivedCardinality,
        inner: &ArchivedItem,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let mut results = Vec::new();

        loop {
            let saved = cursor.pos();
            match self.match_item(inner, cursor) {
                Ok(value) => {
                    if cursor.pos() <= saved {
                        return Err("repeat matched without advancing".into());
                    }
                    results.push(value);
                }
                Err(_) => {
                    cursor.restore(saved);
                    break;
                }
            }
        }

        if matches!(cardinality, ArchivedCardinality::OneOrMore) && results.is_empty() {
            return Err("expected at least one".into());
        }

        self.coerce_repeat(inner, results)
    }

    /// Coerce Vec<Typed> into a typed collection based on the
    /// inner item's content type.
    fn coerce_repeat(
        &self,
        inner: &ArchivedItem,
        items: Vec<Typed>,
    ) -> Result<Typed, String> {
        match &inner.content {
            ArchivedItemContent::DialectRef { target } => {
                match target {
                    ArchivedDialectKind::Expr | ArchivedDialectKind::ExprOr
                    | ArchivedDialectKind::ExprAnd | ArchivedDialectKind::ExprCompare
                    | ArchivedDialectKind::ExprAdd | ArchivedDialectKind::ExprMul
                    | ArchivedDialectKind::ExprPostfix | ArchivedDialectKind::ExprAtom => {
                        let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_expr()).collect();
                        Ok(Typed::Exprs(v?))
                    }
                    ArchivedDialectKind::Statement => {
                        let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_statement()).collect();
                        Ok(Typed::Statements(v?))
                    }
                    ArchivedDialectKind::Param => {
                        let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_param()).collect();
                        Ok(Typed::Params(v?))
                    }
                    ArchivedDialectKind::Type_ => {
                        let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_type_expr()).collect();
                        Ok(Typed::TypeExprs(v?))
                    }
                    ArchivedDialectKind::Pattern => {
                        let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_pattern()).collect();
                        Ok(Typed::Patterns(v?))
                    }
                    _ => {
                        // For other dialect refs in repeats, keep as generic group
                        Ok(Typed::Group(ItemTuple(items)))
                    }
                }
            }
            ArchivedItemContent::Named { label } => {
                if matches!(label.casing, ArchivedCasing::Pascal) {
                    let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_pascal_name()).collect();
                    Ok(Typed::PascalNames(v?))
                } else {
                    let v: Result<Vec<_>, _> = items.into_iter().map(|t| t.into_camel_name()).collect();
                    Ok(Typed::CamelNames(v?))
                }
            }
            ArchivedItemContent::Delimited { .. } => {
                // Repeated delimited groups — keep as ItemTuple
                Ok(Typed::Group(ItemTuple(items)))
            }
            _ => Ok(Typed::Group(ItemTuple(items))),
        }
    }

    // ── Ordered choice matching ─────────────────────────────

    /// Try each alternative in an ordered choice. Match once.
    /// For leaf dialects (Type, Pattern, Param, etc.).
    fn match_one_choice(
        &self,
        alternatives: &rkyv::vec::ArchivedVec<ArchivedAlternative>,
        cursor: &mut Cursor,
    ) -> Result<(usize, ItemTuple), String> {
        for (idx, alt) in alternatives.iter().enumerate() {
            let saved = cursor.pos();
            match self.match_items_seq(&alt.items, cursor) {
                Ok(tuple) => return Ok((idx, tuple)),
                Err(_) => { cursor.restore(saved); }
            }
        }
        Err("no alternative matched".into())
    }

    // ── Helper: first label kind in an alternative ──────────

    fn first_label_kind(items: &rkyv::vec::ArchivedVec<ArchivedItem>) -> Option<&ArchivedLabelKind> {
        for item in items.iter() {
            match &item.content {
                ArchivedItemContent::Named { label } => return Some(&label.kind),
                ArchivedItemContent::Delimited { inner, .. } => {
                    for i in inner.iter() {
                        if let ArchivedItemContent::Named { label } = &i.content {
                            return Some(&label.kind);
                        }
                    }
                }
                ArchivedItemContent::Repeat { inner, .. } => {
                    if let ArchivedItemContent::Named { label } = &inner.content {
                        return Some(&label.kind);
                    }
                    if let ArchivedItemContent::Delimited { inner: d_inner, .. } = &inner.content {
                        for i in d_inner.iter() {
                            if let ArchivedItemContent::Named { label } = &i.content {
                                return Some(&label.kind);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn outer_delim_kind(items: &rkyv::vec::ArchivedVec<ArchivedItem>) -> Option<&ArchivedDelimKind> {
        for item in items.iter() {
            match &item.content {
                ArchivedItemContent::Delimited { kind, .. } => return Some(kind),
                ArchivedItemContent::Repeat { inner, .. } => {
                    if let ArchivedItemContent::Delimited { kind, .. } = &inner.content {
                        return Some(kind);
                    }
                }
                _ => {}
            }
        }
        None
    }

    // ── Token mapping ───────────────────────────────────────

    fn delim_open_token(kind: &ArchivedDelimKind) -> Token {
        match kind {
            ArchivedDelimKind::Paren => Token::LParen,
            ArchivedDelimKind::Bracket => Token::LBracket,
            ArchivedDelimKind::Brace => Token::LBrace,
            ArchivedDelimKind::ParenPipe => Token::LParenPipe,
            ArchivedDelimKind::BracketPipe => Token::LBracketPipe,
            ArchivedDelimKind::BracePipe => Token::LBracePipe,
        }
    }

    fn delim_close_token(kind: &ArchivedDelimKind) -> Token {
        match kind {
            ArchivedDelimKind::Paren => Token::RParen,
            ArchivedDelimKind::Bracket => Token::RBracket,
            ArchivedDelimKind::Brace => Token::RBrace,
            ArchivedDelimKind::ParenPipe => Token::RPipeParen,
            ArchivedDelimKind::BracketPipe => Token::RPipeBracket,
            ArchivedDelimKind::BracePipe => Token::RPipeBrace,
        }
    }

    fn literal_to_token(lit: &ArchivedLiteralToken) -> Token {
        match lit {
            ArchivedLiteralToken::At => Token::At,
            ArchivedLiteralToken::Dollar => Token::Dollar,
            ArchivedLiteralToken::Star => Token::Star,
            ArchivedLiteralToken::Plus => Token::Plus,
            ArchivedLiteralToken::Question => Token::Question,
            ArchivedLiteralToken::Ampersand => Token::Ampersand,
            ArchivedLiteralToken::Caret => Token::Caret,
            ArchivedLiteralToken::Dot => Token::Dot,
            ArchivedLiteralToken::Slash => Token::Slash,
            ArchivedLiteralToken::LogicalOr => Token::LogicalOr,
            ArchivedLiteralToken::LogicalAnd => Token::LogicalAnd,
            ArchivedLiteralToken::Eq => Token::DoubleEquals,
            ArchivedLiteralToken::NotEq => Token::NotEqual,
            ArchivedLiteralToken::Lt => Token::LessThan,
            ArchivedLiteralToken::Gt => Token::GreaterThan,
            ArchivedLiteralToken::LtEq => Token::LessThanOrEqual,
            ArchivedLiteralToken::GtEq => Token::GreaterThanOrEqual,
            ArchivedLiteralToken::Minus => Token::Minus,
            ArchivedLiteralToken::Percent => Token::Percent,
            ArchivedLiteralToken::InlineOr => Token::Pipe,
            ArchivedLiteralToken::Colon => Token::Colon,
            ArchivedLiteralToken::Pipe => Token::Pipe,
            ArchivedLiteralToken::MutAt | ArchivedLiteralToken::BorrowAt => {
                Token::Colon // handled by multi-token match
            }
        }
    }
}

enum BinOp { Or, And }

// ── Per-dialect parse methods ───────────────────────────────
// Each method reads the dialect data and constructs the output
// type directly. No builder. No intermediate bags.

impl Machine {
    // ── Type (leaf) ─────────────────────────────────────────

    fn parse_type(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Type expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let type_expr = match alt_idx {
            0 => {
                // _@_[<TypeApplication>] → InstanceRef
                let _at = tuple.take(0); // Token(@)
                let inner = tuple.take(1).into_group()?;
                let ta = inner.0.into_iter().next()
                    .ok_or("empty type application")?.into_type_app()?;
                TypeExpr::InstanceRef(ta)
            }
            1 => {
                // [<TypeApplication>] → Application
                let inner = tuple.take(0).into_group()?;
                let ta = inner.0.into_iter().next()
                    .ok_or("empty type application")?.into_type_app()?;
                TypeExpr::Application(ta)
            }
            2 => {
                // _$_<GenericParam> → Param or BoundedParam
                let _dollar = tuple.take(0);
                // GenericParam returns either Param or BoundedParam
                match tuple.take(1) {
                    Typed::TypeExpr(te) => te,
                    other => return Err(format!("expected TypeExpr from GenericParam, got {}", other.tag())),
                }
            }
            3 => {
                // :Type → Named
                let (name, _) = tuple.take(0).into_pascal_name()?;
                TypeExpr::Named(TypeName(name))
            }
            _ => return Err("unknown Type alternative".into()),
        };

        Ok(Typed::TypeExpr(type_expr))
    }

    // ── TypeApplication ─────────────────────────────────────

    fn parse_type_application(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // @Constructor +<Type>
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("TypeApplication expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let (constructor, _) = tuple.take(0).into_pascal_name()?;
        let args = tuple.take(1).into_type_exprs()?;

        Ok(Typed::TypeApp(TypeApplication {
            constructor: TypeName(constructor),
            args,
        }))
    }

    // ── GenericParam ────────────────────────────────────────

    fn parse_generic_param(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("GenericParam expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        match alt_idx {
            0 => {
                // @Bound +(_&_ @Bound) → BoundedParam
                let (first_bound, _) = tuple.take(0).into_pascal_name()?;
                let mut bounds = vec![TypeName(first_bound)];
                // Remaining bounds from repeat
                if tuple.len() > 1 {
                    if let Ok(groups) = tuple.take(1).into_group() {
                        for item in groups.0 {
                            if let Ok(g) = item.into_group() {
                                for inner in g.0 {
                                    if let Ok((name, _)) = inner.into_pascal_name() {
                                        bounds.push(TypeName(name));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Typed::TypeExpr(TypeExpr::BoundedParam { bounds }))
            }
            1 => {
                // @Param → simple type param
                let (name, _) = tuple.take(0).into_pascal_name()?;
                Ok(Typed::TypeExpr(TypeExpr::Param(TypeParamName(name))))
            }
            _ => Err("unknown GenericParam alternative".into()),
        }
    }

    // ── Param ───────────────────────────────────────────────

    fn parse_param(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Param expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let param = match alt_idx {
            0 => Param::BorrowSelf,       // _:@_Self
            1 => Param::MutBorrowSelf,    // _~@_Self
            2 => Param::OwnedSelf,        // _@_Self
            3 => {
                // _:@_@Param <Type> → BorrowNamed
                let _borrow = tuple.take(0);
                let (name, _) = tuple.take(1).into_pascal_name()?;
                let typ = tuple.take(2).into_type_expr()?;
                Param::BorrowNamed { name: TypeName(name), typ }
            }
            4 => {
                // _~@_@Param <Type> → MutBorrowNamed
                let _mut = tuple.take(0);
                let (name, _) = tuple.take(1).into_pascal_name()?;
                let typ = tuple.take(2).into_type_expr()?;
                Param::MutBorrowNamed { name: TypeName(name), typ }
            }
            5 => {
                // _@_@Param <Type> → Named
                let _at = tuple.take(0);
                let (name, _) = tuple.take(1).into_pascal_name()?;
                let typ = tuple.take(2).into_type_expr()?;
                Param::Named { name: TypeName(name), typ }
            }
            6 => {
                // _@_@Param → Bare
                let _at = tuple.take(0);
                let (name, _) = tuple.take(1).into_pascal_name()?;
                Param::Bare { name: TypeName(name) }
            }
            _ => return Err("unknown Param alternative".into()),
        };

        Ok(Typed::Param(param))
    }

    // ── Pattern ─────────────────────────────────────────────

    fn parse_pattern(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Pattern expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let pattern = match alt_idx {
            0 => {
                // :Variant _@_ @Binding → VariantDataPattern
                let (variant, span) = tuple.take(0).into_pascal_name()?;
                let _at = tuple.take(1);
                let (binding, _) = tuple.take(2).into_pascal_name()?;
                Pattern::VariantDataPattern {
                    typ: None,
                    variant: VariantName(variant),
                    inner: vec![Pattern::IdentBind {
                        name: TypeName(binding),
                        mutable: false,
                        span: span.clone(),
                    }],
                    span,
                }
            }
            1 => {
                // :Variant → VariantPattern
                let (variant, span) = tuple.take(0).into_pascal_name()?;
                Pattern::VariantPattern {
                    typ: None,
                    variant: VariantName(variant),
                    span,
                }
            }
            2 => {
                // "literal" → StringLitPattern
                let (value, _) = tuple.take(0).into_literal()?;
                match value {
                    LiteralValue::Str(s) => Pattern::StringLitPattern(s),
                    _ => return Err("expected string literal in pattern".into()),
                }
            }
            _ => return Err("unknown Pattern alternative".into()),
        };

        Ok(Typed::Pattern(pattern))
    }

    // ── Signature ───────────────────────────────────────────

    fn parse_signature(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // +<Param> ?<Type>
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Signature expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let params = tuple.take(0).into_params()?;
        let return_type = match tuple.take(1) {
            Typed::TypeExpr(t) => Some(t),
            Typed::TypeExprs(v) if v.is_empty() => None,
            Typed::TypeExprs(mut v) if v.len() == 1 => Some(v.remove(0)),
            Typed::None_ => None,
            _ => None,
        };

        Ok(Typed::MethodSig(MethodSig {
            name: MethodName(String::new()), // filled by caller
            generic_params: vec![],
            params,
            return_type,
            span: Span { start: 0, end: 0 },
        }))
    }

    // ── Body ────────────────────────────────────────────────

    fn parse_body(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // *<Statement> ?<Expr>
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Body expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let statements = tuple.take(0).into_statements().unwrap_or_default();
        let tail = match tuple.take(1) {
            Typed::Expr(e) => Some(Box::new(e)),
            Typed::Exprs(v) if v.is_empty() => None,
            Typed::Exprs(mut v) if v.len() == 1 => Some(Box::new(v.remove(0))),
            Typed::None_ => None,
            _ => None,
        };

        Ok(Typed::Block(Block { statements, tail }))
    }

    // ── Root ─────────────────────────────────────────────────

    fn parse_root(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // Rule 0: Sequential (@Module <Module>)
        let seq_items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Root rule 0 expected Sequential".into()),
        };
        let mut group = self.match_delimited_item(&seq_items[0], cursor)?;
        let (module_name, name_span) = group.take(0).into_pascal_name()?;
        let mut module = group.take(1).into_module()?;
        module.name = TypeName(module_name);
        module.span = name_span;

        // Rule 1: OrderedChoice (repeated)
        let alternatives = match &dialect.rules[1] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Root rule 1 expected OrderedChoice".into()),
        };

        let has_repeat = alternatives.iter().any(|a| {
            matches!(a.cardinality,
                ArchivedCardinality::ZeroOrMore | ArchivedCardinality::OneOrMore)
        });

        loop {
            let mut matched = false;
            for alt in alternatives.iter() {
                let saved = cursor.pos();
                match self.try_root_alt(alt, cursor, &mut module) {
                    Ok(()) => { matched = true; break; }
                    Err(_) => { cursor.restore(saved); }
                }
            }
            if !matched { break; }
            if !has_repeat { break; }
        }

        Ok(Typed::Module(module))
    }

    fn try_root_alt(
        &self,
        alt: &ArchivedAlternative,
        cursor: &mut Cursor,
        module: &mut ModuleDef,
    ) -> Result<(), String> {
        let label_kind = Self::first_label_kind(&alt.items)
            .ok_or("root alt has no label")?;

        match label_kind {
            ArchivedLabelKind::Enum => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_enum_children()?;
                module.enums.push(EnumDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                });
                Ok(())
            }
            ArchivedLabelKind::Struct => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_struct_children()?;
                module.structs.push(StructDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                });
                Ok(())
            }
            ArchivedLabelKind::Trait => {
                let delim = Self::outer_delim_kind(&alt.items);
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_camel_name()?;
                match delim {
                    Some(ArchivedDelimKind::Paren) => {
                        let mut td = group.take(1).into_trait_decl()?;
                        td.name = TraitName(name);
                        td.span = span;
                        module.trait_decls.push(td);
                    }
                    Some(ArchivedDelimKind::Bracket) => {
                        let mut ti = group.take(1).into_trait_impl()?;
                        ti.trait_name = TraitName(name);
                        ti.span = span;
                        module.trait_impls.push(ti);
                    }
                    _ => return Err("unexpected trait delimiter".into()),
                }
                Ok(())
            }
            ArchivedLabelKind::Const => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, name_span) = group.take(0).into_pascal_name()?;
                let typ = group.take(1).into_type_expr()?;
                let (value, val_span) = group.take(2).into_literal()?;
                module.consts.push(ConstDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    typ, value,
                    span: Span { start: name_span.start, end: val_span.end },
                });
                Ok(())
            }
            ArchivedLabelKind::Ffi => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let mut ffi = group.take(1).into_ffi_def()?;
                ffi.library = TypeName(name);
                ffi.span = span;
                module.ffi.push(ffi);
                Ok(())
            }
            ArchivedLabelKind::Newtype => {
                // @Newtype <Type> — two items, not delimited
                let (name, span) = self.match_item(&alt.items[0], cursor)?
                    .into_pascal_name()?;
                let wraps = self.match_item(&alt.items[1], cursor)?
                    .into_type_expr()?;
                module.newtypes.push(NewtypeDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], wraps, span,
                });
                Ok(())
            }
            _ => {
                // Process [|...|] — check if it's a BracketPipe
                if let Some(ArchivedDelimKind::BracketPipe) = Self::outer_delim_kind(&alt.items) {
                    let group = self.match_delimited_item(&alt.items[0], cursor)?;
                    let block = group.0.into_iter().next()
                        .ok_or("empty process")?.into_block()?;
                    module.process = Some(block);
                    return Ok(());
                }
                Err(format!("unknown root construct"))
            }
        }
    }

    // ── Module ──────────────────────────────────────────────

    fn parse_module(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // Module.synth is ONE sequential rule with 3 items:
        // *@ObjectExport *@actionExport *[:Module *:ObjectImport *:actionImport]
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Module expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let mut exports = Vec::new();
        let mut imports = Vec::new();

        // items[0]: *@ObjectExport
        if let Ok(names) = tuple.take(0).into_pascal_names() {
            for (name, _) in names {
                exports.push(ExportItem::Type_(TypeName(name)));
            }
        }

        // items[1]: *@actionExport
        if let Ok(names) = tuple.take(1).into_camel_names() {
            for (name, _) in names {
                exports.push(ExportItem::Trait(TraitName(name)));
            }
        }

        // items[2]: *[:Module *:ObjectImport *:actionImport]
        if let Ok(groups) = tuple.take(2).into_group() {
            for item in groups.0 {
                if let Ok(mut g) = item.into_group() {
                    let (source, _) = g.take(0).into_pascal_name()?;
                    let mut names = Vec::new();
                    if let Ok(objs) = g.take(1).into_pascal_names() {
                        for (n, _) in objs {
                            names.push(ImportItem::Type_(TypeName(n)));
                        }
                    }
                    if let Ok(acts) = g.take(2).into_camel_names() {
                        for (n, _) in acts {
                            names.push(ImportItem::Trait(TraitName(n)));
                        }
                    }
                    imports.push(ModuleImport { source: TypeName(source), names });
                }
            }
        }

        Ok(Typed::Module(ModuleDef {
            name: TypeName(String::new()),
            visibility: Visibility::Public,
            exports, imports,
            enums: vec![], structs: vec![], newtypes: vec![],
            consts: vec![], trait_decls: vec![], trait_impls: vec![],
            ffi: vec![], process: None,
            span: Span { start: 0, end: 0 },
        }))
    }

    // ── Enum ────────────────────────────────────────────────

    fn parse_enum(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Enum expected OrderedChoice".into()),
        };

        let mut children = Vec::new();
        loop {
            let mut matched = false;
            for alt in alternatives.iter() {
                let saved = cursor.pos();
                match self.try_enum_alt(alt, cursor) {
                    Ok(child) => { children.push(child); matched = true; break; }
                    Err(_) => { cursor.restore(saved); }
                }
            }
            if !matched { break; }
        }
        Ok(Typed::EnumChildren(children))
    }

    fn try_enum_alt(
        &self,
        alt: &ArchivedAlternative,
        cursor: &mut Cursor,
    ) -> Result<EnumChild, String> {
        let label = Self::first_label_kind(&alt.items);
        let delim = Self::outer_delim_kind(&alt.items);

        match (label, delim) {
            (Some(ArchivedLabelKind::Variant), None) => {
                // *@Variant → bare
                let (name, span) = self.match_item(&alt.items[0], cursor)?
                    .into_pascal_name()?;
                Ok(EnumChild::Variant { name: VariantName(name), span })
            }
            (Some(ArchivedLabelKind::Variant), Some(ArchivedDelimKind::Paren)) => {
                // *(@Variant <Type>) → data-carrying
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let payload = group.take(1).into_type_expr()?;
                Ok(EnumChild::DataVariant { name: VariantName(name), payload, span })
            }
            (Some(ArchivedLabelKind::Variant), Some(ArchivedDelimKind::Brace)) => {
                // *{@Variant <Struct>} → struct variant
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let struct_children = group.take(1).into_struct_children()?;
                let fields = struct_children.into_iter().filter_map(|c| {
                    match c {
                        StructChild::TypedField { name, visibility, typ, span } =>
                            Some(StructField { name, visibility, typ, span }),
                        _ => None,
                    }
                }).collect();
                Ok(EnumChild::StructVariant { name: VariantName(name), fields, span })
            }
            (Some(ArchivedLabelKind::Enum), Some(ArchivedDelimKind::ParenPipe)) => {
                // *(| @Enum <Enum> |) → nested enum
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_enum_children()?;
                Ok(EnumChild::NestedEnum(EnumDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                }))
            }
            (Some(ArchivedLabelKind::Struct), Some(ArchivedDelimKind::BracePipe)) => {
                // *{| @Struct <Struct> |} → nested struct
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_struct_children()?;
                Ok(EnumChild::NestedStruct(StructDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                }))
            }
            _ => Err("unknown enum alternative".into()),
        }
    }

    // ── Struct ──────────────────────────────────────────────

    fn parse_struct(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Struct expected OrderedChoice".into()),
        };

        let mut children = Vec::new();
        loop {
            let mut matched = false;
            for alt in alternatives.iter() {
                let saved = cursor.pos();
                match self.try_struct_alt(alt, cursor) {
                    Ok(child) => { children.push(child); matched = true; break; }
                    Err(_) => { cursor.restore(saved); }
                }
            }
            if !matched { break; }
        }
        Ok(Typed::StructChildren(children))
    }

    fn try_struct_alt(
        &self,
        alt: &ArchivedAlternative,
        cursor: &mut Cursor,
    ) -> Result<StructChild, String> {
        let label = Self::first_label_kind(&alt.items);
        let delim = Self::outer_delim_kind(&alt.items);

        match (label, delim) {
            (Some(ArchivedLabelKind::Field), Some(ArchivedDelimKind::Paren)) => {
                // *(@Field <Type>) → typed field
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let typ = group.take(1).into_type_expr()?;
                Ok(StructChild::TypedField {
                    name: FieldName(name), visibility: Visibility::Public, typ, span,
                })
            }
            (Some(ArchivedLabelKind::Field), None) => {
                // *@Field → self-typed field
                let (name, span) = self.match_item(&alt.items[0], cursor)?
                    .into_pascal_name()?;
                Ok(StructChild::SelfTypedField {
                    name: FieldName(name), visibility: Visibility::Public, span,
                })
            }
            (Some(ArchivedLabelKind::Enum), Some(ArchivedDelimKind::ParenPipe)) => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_enum_children()?;
                Ok(StructChild::NestedEnum(EnumDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                }))
            }
            (Some(ArchivedLabelKind::Struct), Some(ArchivedDelimKind::BracePipe)) => {
                let mut group = self.match_delimited_item(&alt.items[0], cursor)?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let children = group.take(1).into_struct_children()?;
                Ok(StructChild::NestedStruct(StructDef {
                    name: TypeName(name), visibility: Visibility::Public,
                    generic_params: vec![], derives: vec![], children, span,
                }))
            }
            _ => Err("unknown struct alternative".into()),
        }
    }

    // ── Expression dialects ─────────────────────────────────

    fn parse_expr_binary(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
        _op: BinOp,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("binary expr expected OrderedChoice".into()),
        };

        let last_idx = alternatives.len() - 1;
        for (idx, alt) in alternatives.iter().enumerate() {
            if idx == last_idx {
                // Fallthrough to lower precedence
                return self.match_item(&alt.items[0], cursor);
            }
            let saved = cursor.pos();
            match self.try_binary_alt(alt, cursor) {
                Ok(expr) => return Ok(Typed::Expr(expr)),
                Err(_) => { cursor.restore(saved); }
            }
        }
        Err("no binary expr matched".into())
    }

    fn try_binary_alt(
        &self,
        alt: &ArchivedAlternative,
        cursor: &mut Cursor,
    ) -> Result<Expr, String> {
        let left = self.match_item(&alt.items[0], cursor)?.into_expr()?;
        let op_token = match &alt.items[1].content {
            ArchivedItemContent::Literal { token } => token,
            _ => return Err("expected literal operator".into()),
        };
        let span = cursor.span();
        self.match_literal(op_token, cursor)?;
        let right = self.match_item(&alt.items[2], cursor)?.into_expr()?;
        Ok(Self::make_bin_expr(op_token, left, right, span))
    }

    fn parse_expr_compare(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        self.parse_expr_binary(dialect, cursor, BinOp::Or)
    }

    fn parse_expr_add(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        self.parse_expr_binary(dialect, cursor, BinOp::Or)
    }

    fn parse_expr_mul(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        self.parse_expr_binary(dialect, cursor, BinOp::Or)
    }

    fn make_bin_expr(token: &ArchivedLiteralToken, left: Expr, right: Expr, span: Span) -> Expr {
        let left = Box::new(left);
        let right = Box::new(right);
        match token {
            ArchivedLiteralToken::LogicalOr => Expr::BinOr { left, right, span },
            ArchivedLiteralToken::LogicalAnd => Expr::BinAnd { left, right, span },
            ArchivedLiteralToken::Eq => Expr::BinEq { left, right, span },
            ArchivedLiteralToken::NotEq => Expr::BinNotEq { left, right, span },
            ArchivedLiteralToken::Lt => Expr::BinLt { left, right, span },
            ArchivedLiteralToken::Gt => Expr::BinGt { left, right, span },
            ArchivedLiteralToken::LtEq => Expr::BinLtEq { left, right, span },
            ArchivedLiteralToken::GtEq => Expr::BinGtEq { left, right, span },
            ArchivedLiteralToken::Plus => Expr::BinAdd { left, right, span },
            ArchivedLiteralToken::Minus => Expr::BinSub { left, right, span },
            ArchivedLiteralToken::Star => Expr::BinMul { left, right, span },
            ArchivedLiteralToken::Percent => Expr::BinMod { left, right, span },
            _ => Expr::BinAdd { left, right, span },
        }
    }

    fn parse_expr_atom(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("ExprAtom expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let expr = match alt_idx {
            0 => {
                // _@_:Instance → InstanceRef
                let _at = tuple.take(0);
                let (name, span) = tuple.take(1).into_pascal_name()?;
                Expr::InstanceRef { name: TypeName(name), span }
            }
            1 => {
                // :Variant → BareVariant
                let (name, span) = tuple.take(0).into_pascal_name()?;
                Expr::BareVariant { variant: VariantName(name), span }
            }
            2 => {
                // :Type/:Variant → PathVariant
                let (typ, span) = tuple.take(0).into_pascal_name()?;
                let _slash = tuple.take(1);
                let (variant, vspan) = tuple.take(2).into_pascal_name()?;
                Expr::PathVariant {
                    typ: TypeName(typ), variant: VariantName(variant),
                    span: Span { start: span.start, end: vspan.end },
                }
            }
            3 => {
                // :Type/:method(+<Expr>) → PathCall
                let (typ, span) = tuple.take(0).into_pascal_name()?;
                let _slash = tuple.take(1);
                let (method, _) = tuple.take(2).into_camel_name()?;
                let args_group = tuple.take(3).into_group()?;
                let args = args_group.0.into_iter().next()
                    .map(|t| t.into_exprs()).transpose()?.unwrap_or_default();
                Expr::PathCall {
                    typ: TypeName(typ), method: MethodName(method), args, span,
                }
            }
            4 => {
                // :Literal → literal value
                let (value, span) = tuple.take(0).into_literal()?;
                match value {
                    LiteralValue::Int(v) => Expr::IntLit { value: v, span },
                    LiteralValue::Float(v) => Expr::FloatLit { value: v, span },
                    LiteralValue::Str(v) => Expr::StringLit { value: v, span },
                    LiteralValue::Bool(v) => Expr::BoolLit { value: v, span },
                    LiteralValue::Char(v) => Expr::CharLit { value: v, span },
                }
            }
            5 => {
                // [<Body>] → InlineEval
                let block = tuple.take(0).into_group()?.0.into_iter().next()
                    .ok_or("empty body")?.into_block()?;
                Expr::InlineEval(block)
            }
            6 => {
                // (|<Match>|) → Match
                let m = tuple.take(0).into_group()?.0.into_iter().next()
                    .ok_or("empty match")?.into_match_expr()?;
                Expr::Match(m)
            }
            7 => {
                // [|<Loop>|] → Loop
                let l = tuple.take(0).into_group()?.0.into_iter().next()
                    .ok_or("empty loop")?.into_loop_expr()?;
                Expr::Loop(l)
            }
            8 => {
                // {|<IterationSource> [<Body>]|} → Iteration
                let mut group = tuple.take(0).into_group()?;
                let (source, binding) = group.take(0).into_iter_source()?;
                let body = group.take(1).into_group()?.0.into_iter().next()
                    .ok_or("empty iteration body")?.into_block()?;
                Expr::Iteration(Iteration {
                    binding, source: Box::new(source), body,
                })
            }
            9 => {
                // {<StructConstruct>} → StructConstruct
                let (typ, fields) = tuple.take(0).into_group()?.0.into_iter().next()
                    .ok_or("empty struct construct")?.into_struct_construct()?;
                Expr::StructConstruct {
                    typ, fields, span: Span { start: 0, end: 0 },
                }
            }
            _ => return Err("unknown ExprAtom alternative".into()),
        };

        Ok(Typed::Expr(expr))
    }

    fn parse_postfix(&self, cursor: &mut Cursor) -> Result<Typed, String> {
        let mut base = self.enter_dialect(&ArchivedDialectKind::ExprAtom, cursor)?
            .into_expr()?;

        let dialect = self.lookup(&ArchivedDialectKind::ExprPostfix);
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Ok(Typed::Expr(base)),
        };

        loop {
            let mut matched = false;
            // Try postfix alternatives (skip last which is ExprAtom base)
            for (idx, alt) in alternatives.iter().enumerate() {
                if idx >= alternatives.len() - 1 { break; } // skip base case
                let saved = cursor.pos();
                // Match postfix items starting from index 1 (skip self-ref)
                let mut ok = true;
                let mut postfix = Vec::new();
                for i in 1..alt.items.len() {
                    match self.match_item(&alt.items[i], cursor) {
                        Ok(v) => postfix.push(v),
                        Err(_) => { cursor.restore(saved); ok = false; break; }
                    }
                }
                if ok && cursor.pos() > saved {
                    let label = Self::first_label_kind_at(&alt.items, 1);
                    let start = Span { start: 0, end: 0 };
                    base = match label {
                        Some(ArchivedLabelKind::Field) => {
                            let (field, span) = postfix.into_iter().nth(1)
                                .ok_or("missing field name")?.into_pascal_name()?;
                            Expr::FieldAccess {
                                object: Box::new(base), field: FieldName(field), span,
                            }
                        }
                        Some(ArchivedLabelKind::Method) => {
                            let _dot = postfix.remove(0);
                            let (method, _) = postfix.remove(0).into_camel_name()?;
                            let args = if !postfix.is_empty() {
                                postfix.remove(0).into_group()
                                    .ok().and_then(|mut g| {
                                        g.0.pop().and_then(|t| t.into_exprs().ok())
                                    })
                                    .unwrap_or_default()
                            } else { vec![] };
                            Expr::MethodCall {
                                object: Box::new(base),
                                method: MethodName(method), args,
                                span: start,
                            }
                        }
                        _ => {
                            // _?_ → TryUnwrap
                            Expr::TryUnwrap {
                                inner: Box::new(base),
                                span: postfix.into_iter().next()
                                    .map(|t| match t { Typed::Token(s) => s, _ => start.clone() })
                                    .unwrap_or(start),
                            }
                        }
                    };
                    matched = true;
                    break;
                }
            }
            if !matched { break; }
        }

        Ok(Typed::Expr(base))
    }

    fn first_label_kind_at(items: &rkyv::vec::ArchivedVec<ArchivedItem>, start: usize) -> Option<&ArchivedLabelKind> {
        for i in start..items.len() {
            if let ArchivedItemContent::Named { label } = &items[i].content {
                return Some(&label.kind);
            }
        }
        None
    }

    // ── Statement ───────────────────────────────────────────

    fn parse_statement(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Statement expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let stmt = match alt_idx {
            0 => {
                // ^<Expr> → EarlyReturn
                let _caret = tuple.take(0);
                let expr = tuple.take(1).into_expr()?;
                Statement::EarlyReturn(Box::new(expr))
            }
            1 => {
                // [|<Expr> <Body>|] → While
                let mut group = tuple.take(0).into_group()?;
                let cond = group.take(0).into_expr()?;
                let body = group.take(1).into_block()?;
                Statement::Loop(LoopExpr {
                    condition: Some(Box::new(cond)), body,
                })
            }
            2 => {
                // [|<Body>|] → infinite loop
                let group = tuple.take(0).into_group()?;
                let body = group.0.into_iter().next()
                    .ok_or("empty loop body")?.into_block()?;
                Statement::Loop(LoopExpr { condition: None, body })
            }
            3 => {
                // {|<IterationSource> [<Body>]|} → Iteration
                let mut group = tuple.take(0).into_group()?;
                let (source, binding) = group.take(0).into_iter_source()?;
                let body = group.take(1).into_group()?.0.into_iter().next()
                    .ok_or("empty iteration body")?.into_block()?;
                Statement::Iteration(Iteration {
                    binding, source: Box::new(source), body,
                })
            }
            4 => {
                // (@Type <Type>) → LocalTypeDecl
                let mut group = tuple.take(0).into_group()?;
                let (name, span) = group.take(0).into_pascal_name()?;
                let typ = group.take(1).into_type_expr()?;
                Statement::LocalTypeDecl { name: TypeName(name), typ, span }
            }
            5 => {
                // _~@_<Mutation> → Mutation
                let _mut_at = tuple.take(0);
                let mutation = tuple.take(1).into_mutation()?;
                Statement::Mutation(mutation)
            }
            6 => {
                // _@_<Instance> → Instance
                let _at = tuple.take(0);
                let instance = tuple.take(1).into_instance()?;
                Statement::Instance(instance)
            }
            7 => {
                // <Expr> → expression statement
                let expr = tuple.take(0).into_expr()?;
                Statement::Expr(Box::new(expr))
            }
            _ => return Err("unknown Statement alternative".into()),
        };

        Ok(Typed::Statement(stmt))
    }

    // ── Instance ────────────────────────────────────────────

    fn parse_instance(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Instance expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let instance = match alt_idx {
            0 => {
                // @Instance (<Type>) <Expr>
                let (name, span) = tuple.take(0).into_pascal_name()?;
                let type_ann = tuple.take(1).into_group()?.0.into_iter().next()
                    .map(|t| t.into_type_expr()).transpose()?;
                let value = tuple.take(2).into_expr()?;
                Instance {
                    name: TypeName(name), type_annotation: type_ann,
                    value: Box::new(value), span,
                }
            }
            1 => {
                // @Instance <Expr>
                let (name, span) = tuple.take(0).into_pascal_name()?;
                let value = tuple.take(1).into_expr()?;
                Instance {
                    name: TypeName(name), type_annotation: None,
                    value: Box::new(value), span,
                }
            }
            _ => return Err("unknown Instance alternative".into()),
        };

        Ok(Typed::Instance(instance))
    }

    // ── Mutation ────────────────────────────────────────────

    fn parse_mutation(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // :Instance.:method(+<Expr>)
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Mutation expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let (name, span) = tuple.take(0).into_pascal_name()?;
        let _dot = tuple.take(1);
        let (method, _) = tuple.take(2).into_camel_name()?;
        let args_group = tuple.take(3).into_group()?;
        let args = args_group.0.into_iter().next()
            .map(|t| t.into_exprs()).transpose()?.unwrap_or_default();

        Ok(Typed::Mutation(Mutation {
            name: TypeName(name), method: MethodName(method), args, span,
        }))
    }

    // ── IterationSource ─────────────────────────────────────

    fn parse_iteration_source(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // <Expr>.@Binding
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("IterationSource expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let source = tuple.take(0).into_expr()?;
        let _dot = tuple.take(1);
        let (binding, span) = tuple.take(2).into_pascal_name()?;

        Ok(Typed::IterSource {
            source,
            binding: Pattern::IdentBind {
                name: TypeName(binding), mutable: false, span,
            },
        })
    }

    // ── StructConstruct ─────────────────────────────────────

    fn parse_struct_construct(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // :Struct +(:Field <Expr>)
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("StructConstruct expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let (typ, _) = tuple.take(0).into_pascal_name()?;

        let mut fields = Vec::new();
        if let Ok(groups) = tuple.take(1).into_group() {
            for item in groups.0 {
                if let Ok(mut g) = item.into_group() {
                    let (field_name, _) = g.take(0).into_pascal_name()?;
                    let value = g.take(1).into_expr()?;
                    fields.push(FieldInit {
                        name: FieldName(field_name), value: Box::new(value),
                    });
                }
            }
        }

        Ok(Typed::StructConstruct { typ: TypeName(typ), fields })
    }

    // ── Match ───────────────────────────────────────────────

    fn parse_match(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // ?<Expr> +(<Pattern>) <Expr>
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Match expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;

        // Optional target expression
        let target = match tuple.take(0) {
            Typed::Expr(e) => Some(Box::new(e)),
            Typed::Exprs(v) if v.is_empty() => None,
            Typed::Exprs(mut v) if v.len() == 1 => Some(Box::new(v.remove(0))),
            _ => None,
        };

        // Arms: repeated (pattern) expr pairs
        let mut arms = Vec::new();
        if let Ok(groups) = tuple.take(1).into_group() {
            for item in groups.0 {
                if let Ok(mut g) = item.into_group() {
                    // Each group: Seq([pattern_group, expr])
                    // Actually it's from +(<Pattern>) <Expr>, so...
                    // The repeat contains delimited (Pattern) followed by Expr
                    // This needs more careful handling
                    let pattern = g.take(0).into_group()?.0.into_iter().next()
                        .ok_or("empty pattern")?.into_pattern()?;
                    let result = g.take(1).into_expr()?;
                    arms.push(MatchArm {
                        pattern, guard: None, result: Box::new(result),
                    });
                }
            }
        }

        Ok(Typed::MatchExpr(MatchExpr { target, arms }))
    }

    // ── Loop ────────────────────────────────────────────────

    fn parse_loop(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        let alternatives = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Loop expected OrderedChoice".into()),
        };

        let (alt_idx, mut tuple) = self.match_one_choice(alternatives, cursor)?;

        let loop_expr = match alt_idx {
            0 => {
                // <Expr> +<Statement> → conditional
                let cond = tuple.take(0).into_expr()?;
                let stmts = tuple.take(1).into_statements()?;
                LoopExpr {
                    condition: Some(Box::new(cond)),
                    body: Block { statements: stmts, tail: None },
                }
            }
            1 => {
                // +<Statement> → infinite
                let stmts = tuple.take(0).into_statements()?;
                LoopExpr {
                    condition: None,
                    body: Block { statements: stmts, tail: None },
                }
            }
            _ => return Err("unknown Loop alternative".into()),
        };

        Ok(Typed::LoopExpr(loop_expr))
    }

    // ── Process ─────────────────────────────────────────────

    fn parse_process(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // +<Statement>
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Process expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let stmts = tuple.take(0).into_statements()?;

        Ok(Typed::Block(Block { statements: stmts, tail: None }))
    }

    // ── Method ──────────────────────────────────────────────

    fn parse_method(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // +<Param> ?<Type> // body alternatives
        // Method.synth has 2 rules: Sequential then OrderedChoice

        // Rule 0: Sequential (+<Param> ?<Type>)
        let seq_items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Method rule 0 expected Sequential".into()),
        };

        let mut seq_tuple = self.match_items_seq(seq_items, cursor)?;
        let params = seq_tuple.take(0).into_params()?;
        let return_type = match seq_tuple.take(1) {
            Typed::TypeExpr(t) => Some(t),
            Typed::TypeExprs(v) if v.is_empty() => None,
            Typed::TypeExprs(mut v) if v.len() == 1 => Some(v.remove(0)),
            _ => None,
        };

        // Rule 1: OrderedChoice (body variants)
        let alternatives = match &dialect.rules[1] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err("Method rule 1 expected OrderedChoice".into()),
        };

        let (_body_idx, mut body_tuple) = self.match_one_choice(alternatives, cursor)?;

        let body = match body_tuple.take(0) {
            Typed::Block(b) => MethodBody::Block(b),
            Typed::MatchExpr(m) => MethodBody::Match(m),
            Typed::LoopExpr(l) => MethodBody::Loop(l),
            Typed::Iteration(i) => MethodBody::Iteration(i),
            Typed::Group(mut g) => {
                // Could be delimited body — try to extract
                match g.take(0) {
                    Typed::Block(b) => MethodBody::Block(b),
                    Typed::MatchExpr(m) => MethodBody::Match(m),
                    Typed::LoopExpr(l) => MethodBody::Loop(l),
                    other => {
                        if let Ok((typ, fields)) = other.into_struct_construct() {
                            MethodBody::StructConstruct {
                                typ, fields, span: Span { start: 0, end: 0 },
                            }
                        } else {
                            return Err("unknown method body".into());
                        }
                    }
                }
            }
            other => return Err(format!("unexpected method body: {}", other.tag())),
        };

        Ok(Typed::MethodDef(MethodDef {
            name: MethodName(String::new()), // filled by caller
            generic_params: vec![],
            params, return_type, body,
            span: Span { start: 0, end: 0 },
        }))
    }

    // ── TraitDecl ───────────────────────────────────────────

    fn parse_trait_decl(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // [+(@signature <Signature>)]
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("TraitDecl expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let sigs_group = tuple.take(0).into_group()?;

        let mut signatures = Vec::new();
        // The group contains the repeated (@signature <Signature>) pairs
        if let Some(Typed::Group(inner_groups)) = sigs_group.0.into_iter().next() {
            for item in inner_groups.0 {
                if let Ok(mut g) = item.into_group() {
                    let (name, span) = g.take(0).into_camel_name()?;
                    let mut sig = g.take(1).into_method_sig()?;
                    sig.name = MethodName(name);
                    sig.span = span;
                    signatures.push(sig);
                }
            }
        }

        Ok(Typed::TraitDecl(TraitDeclDef {
            name: TraitName(String::new()),
            visibility: Visibility::Public,
            generic_params: vec![], super_traits: vec![],
            associated_types: vec![], signatures,
            span: Span { start: 0, end: 0 },
        }))
    }

    // ── TraitImpl ───────────────────────────────────────────

    fn parse_trait_impl(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // <Type> [<TypeImpl>]
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("TraitImpl expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let typ = tuple.take(0).into_type_expr()?;
        let methods_group = tuple.take(1).into_group()?;
        let methods = methods_group.0.into_iter().next()
            .map(|t| t.into_methods()).transpose()?.unwrap_or_default();

        Ok(Typed::TraitImpl(TraitImplDef {
            trait_name: TraitName(String::new()),
            trait_args: vec![], typ,
            generic_params: vec![], methods,
            associated_types: vec![],
            span: Span { start: 0, end: 0 },
        }))
    }

    // ── TypeImpl ────────────────────────────────────────────

    fn parse_type_impl(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // +(@method <Method>)
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("TypeImpl expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let mut methods = Vec::new();

        if let Ok(groups) = tuple.take(0).into_group() {
            for item in groups.0 {
                if let Ok(mut g) = item.into_group() {
                    let (name, span) = g.take(0).into_camel_name()?;
                    let mut method = g.take(1).into_method_def()?;
                    method.name = MethodName(name);
                    method.span = span;
                    methods.push(method);
                }
            }
        }

        Ok(Typed::Methods(methods))
    }

    // ── Ffi ─────────────────────────────────────────────────

    fn parse_ffi(
        &self,
        dialect: &ArchivedDialect,
        cursor: &mut Cursor,
    ) -> Result<Typed, String> {
        // +(@foreignFunction <Signature>)
        let items = match &dialect.rules[0] {
            ArchivedRule::Sequential { items } => items,
            _ => return Err("Ffi expected Sequential".into()),
        };

        let mut tuple = self.match_items_seq(items, cursor)?;
        let mut functions = Vec::new();

        if let Ok(groups) = tuple.take(0).into_group() {
            for item in groups.0 {
                if let Ok(mut g) = item.into_group() {
                    let (name, span) = g.take(0).into_camel_name()?;
                    let mut sig = g.take(1).into_method_sig()?;
                    functions.push(FfiFunction {
                        name: MethodName(name),
                        params: sig.params,
                        return_type: sig.return_type,
                        span,
                    });
                }
            }
        }

        Ok(Typed::FfiDef(FfiDef {
            library: TypeName(String::new()),
            functions,
            span: Span { start: 0, end: 0 },
        }))
    }
}
