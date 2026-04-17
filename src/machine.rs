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

    // Placeholder stubs for dialects not yet ported.
    // These will be filled in as we go.

    fn parse_root(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("root not yet ported".into()) }
    fn parse_module(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("module not yet ported".into()) }
    fn parse_enum(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("enum not yet ported".into()) }
    fn parse_struct(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("struct not yet ported".into()) }
    fn parse_statement(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("statement not yet ported".into()) }
    fn parse_instance(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("instance not yet ported".into()) }
    fn parse_mutation(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("mutation not yet ported".into()) }
    fn parse_method(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("method not yet ported".into()) }
    fn parse_trait_decl(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("trait_decl not yet ported".into()) }
    fn parse_trait_impl(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("trait_impl not yet ported".into()) }
    fn parse_type_impl(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("type_impl not yet ported".into()) }
    fn parse_match(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("match not yet ported".into()) }
    fn parse_loop(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("loop not yet ported".into()) }
    fn parse_process(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("process not yet ported".into()) }
    fn parse_iteration_source(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("iteration_source not yet ported".into()) }
    fn parse_struct_construct(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("struct_construct not yet ported".into()) }
    fn parse_ffi(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("ffi not yet ported".into()) }
    fn parse_expr_binary(&self, _d: &ArchivedDialect, _c: &mut Cursor, _op: BinOp) -> Result<Typed, String> { Err("expr_binary not yet ported".into()) }
    fn parse_expr_compare(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("expr_compare not yet ported".into()) }
    fn parse_expr_add(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("expr_add not yet ported".into()) }
    fn parse_expr_mul(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("expr_mul not yet ported".into()) }
    fn parse_expr_atom(&self, _d: &ArchivedDialect, _c: &mut Cursor) -> Result<Typed, String> { Err("expr_atom not yet ported".into()) }
    fn parse_postfix(&self, _c: &mut Cursor) -> Result<Typed, String> { Err("postfix not yet ported".into()) }
}
