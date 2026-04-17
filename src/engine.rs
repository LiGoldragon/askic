/// Engine — generic dialect state machine.
///
/// Walks the ArchivedDialectTree, matches tokens, produces ParseValues.
/// Has NO knowledge of sema-core types — that's the builder's job.

use std::collections::HashMap;

use aski_core::*;
use crate::lexer::{Token, Spanned};
use crate::values::*;
use crate::builder::Builder;

pub struct Engine {
    data: &'static [u8],
    dialect_index: HashMap<u8, usize>,
}

impl Engine {
    pub fn new(data: &'static [u8]) -> Self {
        let tree = Self::access_tree(data);
        let mut dialect_index = HashMap::new();
        for (i, dialect) in tree.dialects.iter().enumerate() {
            let disc = Self::dialect_discriminant(&dialect.kind);
            dialect_index.insert(disc, i);
        }
        Engine { data, dialect_index }
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

    pub fn lookup(&self, kind: &ArchivedDialectKind) -> &ArchivedDialect {
        let disc = Self::dialect_discriminant(kind);
        let idx = self.dialect_index.get(&disc)
            .expect("dialect not found");
        &self.tree().dialects[*idx]
    }

    pub fn parse(&self, tokens: &[Spanned]) -> Result<Vec<sema_core::RootChild>, String> {
        let mut cursor = Cursor::new(tokens);
        let builder = Builder::new();
        let result = self.enter_dialect(&ArchivedDialectKind::Root, &mut cursor, &builder)?;
        match result {
            ParseValue::Dialect(DialectValue::RootChildren(children)) => Ok(children),
            other => Err(format!("root returned unexpected: {:?}", other)),
        }
    }

    pub fn enter_dialect(
        &self,
        kind: &ArchivedDialectKind,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<ParseValue, String> {
        // Pass-through: Expr just delegates to ExprOr
        if matches!(kind, ArchivedDialectKind::Expr) {
            return self.enter_dialect(&ArchivedDialectKind::ExprOr, cursor, builder);
        }

        // ExprPostfix is left-recursive — handle iteratively
        if matches!(kind, ArchivedDialectKind::ExprPostfix) {
            return self.parse_postfix(cursor, builder);
        }

        let dialect = self.lookup(kind);
        let mut matched_rules = Vec::new();

        for rule in dialect.rules.iter() {
            match rule {
                ArchivedRule::Sequential { items } => {
                    let values = self.match_items(items, cursor, builder)?;
                    matched_rules.push(MatchedRule::Sequential(values));
                }
                ArchivedRule::OrderedChoice { alternatives } => {
                    let result = self.match_repeated_choice(alternatives, cursor, builder)?;
                    matched_rules.push(result);
                }
            }
        }

        builder.build(kind, matched_rules)
    }

    fn match_repeated_choice(
        &self,
        alternatives: &rkyv::vec::ArchivedVec<ArchivedAlternative>,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<MatchedRule, String> {
        let mut collected: Vec<(usize, Vec<ParseValue>)> = Vec::new();

        loop {
            let mut matched_any = false;

            for (alt_idx, alt) in alternatives.iter().enumerate() {
                let saved = cursor.pos();
                match self.match_items(&alt.items, cursor, builder) {
                    Ok(values) => {
                        collected.push((alt_idx, values));
                        matched_any = true;
                        break; // restart from first alternative
                    }
                    Err(_) => {
                        cursor.restore(saved);
                    }
                }
            }

            if !matched_any { break; }

            // Check cardinality — if any alt was OneOrMore or ZeroOrMore, keep looping.
            // If all remaining are Optional or One, and we matched, we're done for One/Optional.
            // For simplicity: always loop. The break condition is "no alt matched."
        }

        if collected.is_empty() {
            // Check if any alternative allows zero matches
            let allows_empty = alternatives.iter().any(|a| {
                matches!(a.cardinality, ArchivedCardinality::ZeroOrMore | ArchivedCardinality::Optional)
            });
            if allows_empty {
                return Ok(MatchedRule::RepeatedChoice(collected));
            }
            return Err("no alternative matched".into());
        }

        // If we collected exactly one and no alternative has * or + cardinality,
        // return as a simple Choice
        if collected.len() == 1 {
            let has_repeat = alternatives.iter().any(|a| {
                matches!(a.cardinality, ArchivedCardinality::ZeroOrMore | ArchivedCardinality::OneOrMore)
            });
            if !has_repeat {
                let (idx, values) = collected.into_iter().next().unwrap();
                return Ok(MatchedRule::Choice(idx, values));
            }
        }

        Ok(MatchedRule::RepeatedChoice(collected))
    }

    fn match_items(
        &self,
        items: &rkyv::vec::ArchivedVec<ArchivedItem>,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<Vec<ParseValue>, String> {
        let mut values = Vec::new();
        for item in items.iter() {
            values.push(self.match_item(item, cursor, builder)?);
        }
        Ok(values)
    }

    fn match_item(
        &self,
        item: &ArchivedItem,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<ParseValue, String> {
        // Check adjacency
        if item.adjacent && !cursor.is_adjacent() {
            return Err("expected adjacent token".into());
        }

        match &item.content {
            ArchivedItemContent::Named { label } => {
                self.match_named(label, cursor)
            }
            ArchivedItemContent::DialectRef { target } => {
                self.enter_dialect(target, cursor, builder)
            }
            ArchivedItemContent::Delimited { kind, inner } => {
                self.match_delimited(kind, inner, cursor, builder)
            }
            ArchivedItemContent::Literal { token } => {
                self.match_literal(token, cursor)
            }
            ArchivedItemContent::Keyword { token } => {
                self.match_keyword(token, cursor)
            }
            ArchivedItemContent::Repeat { kind, inner } => {
                self.match_repeat(kind, inner, cursor, builder)
            }
            ArchivedItemContent::LiteralValue => {
                self.match_literal_value(cursor)
            }
        }
    }

    fn match_named(
        &self,
        label: &ArchivedLabel,
        cursor: &mut Cursor,
    ) -> Result<ParseValue, String> {
        let is_pascal = matches!(label.casing, ArchivedCasing::Pascal);
        let tok = cursor.peek().ok_or("unexpected end of tokens")?;

        let name = match &tok.token {
            Token::PascalIdent(s) if is_pascal => s.clone(),
            Token::CamelIdent(s) if !is_pascal => s.clone(),
            other => return Err(format!("expected {} name, got {:?}",
                if is_pascal { "PascalCase" } else { "camelCase" }, other)),
        };

        let span = cursor.span();
        cursor.advance();
        Ok(ParseValue::Name(name, span))
    }

    fn match_delimited(
        &self,
        kind: &ArchivedDelimKind,
        inner: &rkyv::vec::ArchivedVec<ArchivedItem>,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<ParseValue, String> {
        let open = Self::delim_open_token(kind);
        let close = Self::delim_close_token(kind);

        cursor.expect(&open)?;
        let values = self.match_items(inner, cursor, builder)?;
        cursor.expect(&close)?;

        Ok(ParseValue::Seq(values))
    }

    fn match_literal(
        &self,
        token: &ArchivedLiteralToken,
        cursor: &mut Cursor,
    ) -> Result<ParseValue, String> {
        let expected = Self::literal_to_token(token);
        let span = cursor.span();
        cursor.expect(&expected)?;
        Ok(ParseValue::Token(span))
    }

    fn match_keyword(
        &self,
        keyword: &ArchivedKeywordToken,
        cursor: &mut Cursor,
    ) -> Result<ParseValue, String> {
        let expected = match keyword {
            ArchivedKeywordToken::Self_ => "Self",
            ArchivedKeywordToken::Main => "Main",
        };
        let tok = cursor.peek().ok_or("unexpected end of tokens")?;
        match &tok.token {
            Token::PascalIdent(s) if s == expected => {
                let span = cursor.span();
                cursor.advance();
                Ok(ParseValue::Keyword(span))
            }
            other => Err(format!("expected keyword {}, got {:?}", expected, other)),
        }
    }

    fn match_repeat(
        &self,
        cardinality: &ArchivedCardinality,
        inner: &ArchivedItem,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<ParseValue, String> {
        let mut results = Vec::new();

        loop {
            let saved = cursor.pos();
            match self.match_item(inner, cursor, builder) {
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

        match cardinality {
            ArchivedCardinality::OneOrMore if results.is_empty() => {
                Err("expected at least one".into())
            }
            _ => Ok(ParseValue::Seq(results)),
        }
    }

    fn match_literal_value(
        &self,
        cursor: &mut Cursor,
    ) -> Result<ParseValue, String> {
        let tok = cursor.peek().ok_or("unexpected end of tokens")?;
        let span = cursor.span();
        match &tok.token {
            Token::Integer(v) => {
                let val = sema_core::LiteralValue::Int(*v);
                cursor.advance();
                Ok(ParseValue::Literal(val, span))
            }
            Token::Float(s) => {
                let val = sema_core::LiteralValue::Float(s.parse().unwrap_or(0.0));
                cursor.advance();
                Ok(ParseValue::Literal(val, span))
            }
            Token::StringLit(s) => {
                let val = sema_core::LiteralValue::Str(s.clone());
                cursor.advance();
                Ok(ParseValue::Literal(val, span))
            }
            other => Err(format!("expected literal value, got {:?}", other)),
        }
    }

    fn parse_postfix(
        &self,
        cursor: &mut Cursor,
        builder: &Builder,
    ) -> Result<ParseValue, String> {
        // Base case: parse ExprAtom
        let mut result = self.enter_dialect(&ArchivedDialectKind::ExprAtom, cursor, builder)?;

        // Loop: try postfix operations (alts 0-2 of ExprPostfix)
        // We re-lookup the dialect each iteration to avoid holding a borrow across match_item
        loop {
            let mut matched = false;

            // Try alts 0-2 (skip alt 3 which is the ExprAtom base case)
            for alt_idx in 0..3 {
                let saved = cursor.pos();

                // Re-lookup each time to avoid borrow conflict
                let dialect = self.lookup(&ArchivedDialectKind::ExprPostfix);
                let alternatives = match &dialect.rules[0] {
                    ArchivedRule::OrderedChoice { alternatives } => alternatives,
                    _ => return Ok(result),
                };

                if alt_idx >= alternatives.len() { break; }
                let alt = &alternatives[alt_idx];

                // Collect item references we need, then drop the borrow
                let item_count = alt.items.len();
                if item_count <= 1 { continue; } // skip if only <ExprPostfix>

                let _ = dialect;

                // Now match the postfix items (skip first which is <ExprPostfix>)
                let mut postfix_values = Vec::new();
                let mut ok = true;

                for item_idx in 1..item_count {
                    let dialect = self.lookup(&ArchivedDialectKind::ExprPostfix);
                    let alts = match &dialect.rules[0] {
                        ArchivedRule::OrderedChoice { alternatives } => alternatives,
                        _ => { ok = false; break; }
                    };
                    let item = &alts[alt_idx].items[item_idx];

                    match self.match_item(item, cursor, builder) {
                        Ok(v) => postfix_values.push(v),
                        Err(_) => { cursor.restore(saved); ok = false; break; }
                    }
                }

                if ok {
                    result = builder.build_postfix(alt_idx, result, postfix_values)?;
                    matched = true;
                    break;
                }
            }

            if !matched { break; }
        }

        Ok(result)
    }

    // ── Token mapping helpers ───────────────────────────────

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
            ArchivedLiteralToken::MutAt => Token::Tilde, // ~@ is two tokens: ~ then @
            ArchivedLiteralToken::BorrowAt => Token::Colon, // :@ is two tokens: : then @
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
            ArchivedLiteralToken::InlineOr => Token::LogicalOr, // TODO: context-dependent
            ArchivedLiteralToken::Colon => Token::Colon,
            ArchivedLiteralToken::Pipe => Token::Pipe,
        }
    }
}

/// Token cursor with position tracking and adjacency detection.
pub struct Cursor<'a> {
    tokens: &'a [Spanned],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(tokens: &'a [Spanned]) -> Self {
        Cursor { tokens, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn restore(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn peek(&self) -> Option<&Spanned> {
        self.skip_newlines_pos().and_then(|p| self.tokens.get(p))
    }

    pub fn advance(&mut self) {
        if let Some(p) = self.skip_newlines_pos() {
            self.pos = p + 1;
        }
    }

    pub fn span(&self) -> sema_core::Span {
        if let Some(tok) = self.peek() {
            sema_core::Span {
                start: tok.span.start as u32,
                end: tok.span.end as u32,
            }
        } else {
            sema_core::Span { start: 0, end: 0 }
        }
    }

    pub fn is_adjacent(&self) -> bool {
        if self.pos == 0 { return true; }
        if let (Some(prev), Some(curr)) = (
            self.tokens.get(self.pos.saturating_sub(1)),
            self.peek()
        ) {
            curr.span.start == prev.span.end
        } else {
            false
        }
    }

    pub fn at_end(&self) -> bool {
        self.skip_newlines_pos().is_none()
    }

    pub fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.peek().ok_or_else(|| format!("expected {:?}, got EOF", expected))?;
        if &tok.token == expected {
            self.advance();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", expected, tok.token))
        }
    }

    fn skip_newlines_pos(&self) -> Option<usize> {
        let mut p = self.pos;
        while p < self.tokens.len() {
            if !matches!(self.tokens[p].token, Token::Newline) {
                return Some(p);
            }
            p += 1;
        }
        None
    }
}
