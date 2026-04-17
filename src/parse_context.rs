/// Parse context — per-parse mutable state + all matching methods.
///
/// All methods on ParseContext. No free functions.

use aski_core::*;
use sema_core::Span;
use crate::lexer::Token;
use crate::token_stream::TokenStream;
use crate::engine::Engine;
use crate::error::ParseError;
use crate::values::{ParseValue, DialectValue, MatchedRule};

pub struct ParseContext<'a> {
    engine: &'a Engine,
    stream: TokenStream,
    context_stack: Vec<String>,
}

impl<'a> ParseContext<'a> {
    pub fn new(engine: &'a Engine, stream: TokenStream) -> Self {
        ParseContext { engine, stream, context_stack: Vec::new() }
    }

    // ── Dialect entry ───────────────────────────────────────

    pub fn parse_dialect(&mut self, kind: &ArchivedDialectKind) -> Result<ParseValue, ParseError> {
        // Pass-through: Expr → ExprOr
        if matches!(kind, ArchivedDialectKind::Expr) {
            return self.parse_dialect(&ArchivedDialectKind::ExprOr);
        }

        // Left-recursive: ExprPostfix
        if self.engine.is_left_recursive(kind) {
            return self.parse_left_recursive(kind);
        }

        let dialect = self.engine.lookup(kind);
        self.context_stack.push(format!("{:?}", kind));

        let mut rule_results = Vec::new();
        for rule in dialect.rules.iter() {
            let result = match rule {
                ArchivedRule::Sequential { items } => {
                    self.stream.skip_newlines();
                    MatchedRule::Sequential(self.match_items(items)?)
                }
                ArchivedRule::OrderedChoice { alternatives } => {
                    let has_repeat = alternatives.iter()
                        .any(|a| !matches!(a.cardinality, ArchivedCardinality::One));
                    if has_repeat {
                        MatchedRule::RepeatedChoice(self.match_repeated_choice(alternatives)?)
                    } else {
                        let (idx, vals) = self.match_ordered_choice(alternatives)?;
                        MatchedRule::Choice(idx, vals)
                    }
                }
            };
            rule_results.push(result);
        }

        self.context_stack.pop();
        self.build_from_dialect(kind, rule_results)
    }

    // ── Left-recursive handling (ExprPostfix) ───────────────

    fn parse_left_recursive(&mut self, kind: &ArchivedDialectKind) -> Result<ParseValue, ParseError> {
        let dialect = self.engine.lookup(kind);
        self.context_stack.push(format!("{:?}", kind));

        // Find the non-recursive base case (last alternative)
        let alts = match &dialect.rules[0] {
            ArchivedRule::OrderedChoice { alternatives } => alternatives,
            _ => return Err(self.error("left-recursive dialect must have ordered choice")),
        };

        // Parse base case (last alternative — the non-recursive one)
        let base_alt = &alts[alts.len() - 1];
        let mut result = self.match_items(&base_alt.items)?;

        // Iteratively try postfix alternatives
        loop {
            let saved = self.stream.save();
            let mut matched = false;

            for (idx, alt) in alts.iter().enumerate() {
                if idx == alts.len() - 1 { continue; } // skip base case

                // Skip the self-recursive first item, match remaining
                let remaining = &alt.items[1..];
                match self.try_match_items(remaining) {
                    Ok(postfix_values) => {
                        result = self.build_postfix(kind, idx, result, postfix_values)?;
                        matched = true;
                        break;
                    }
                    Err(_) => {
                        self.stream.restore(saved);
                    }
                }
            }

            if !matched { break; }
        }

        self.context_stack.pop();
        Ok(result)
    }

    // ── Ordered choice ──────────────────────────────────────

    fn match_ordered_choice(
        &mut self,
        alternatives: &[ArchivedAlternative],
    ) -> Result<(usize, Vec<ParseValue>), ParseError> {
        self.stream.skip_newlines();
        let saved = self.stream.save();
        let mut best_error: Option<ParseError> = None;
        let mut best_pos: usize = 0;

        for (idx, alt) in alternatives.iter().enumerate() {
            match self.try_match_items(&alt.items) {
                Ok(values) => return Ok((idx, values)),
                Err(e) => {
                    let reached = self.stream.save();
                    self.stream.restore(saved);
                    if reached >= best_pos {
                        best_pos = reached;
                        best_error = Some(e);
                    }
                }
            }
        }

        Err(best_error.unwrap_or_else(|| self.error("no alternatives matched")))
    }

    fn match_repeated_choice(
        &mut self,
        alternatives: &[ArchivedAlternative],
    ) -> Result<Vec<(usize, Vec<ParseValue>)>, ParseError> {
        let mut all_results = Vec::new();

        loop {
            self.stream.skip_newlines();
            if self.stream.at_end() { break; }

            let saved = self.stream.save();
            let mut iteration_matched = false;

            for (idx, alt) in alternatives.iter().enumerate() {
                let results = self.try_match_with_cardinality(alt);
                match results {
                    Ok(matches) if !matches.is_empty() => {
                        for m in matches {
                            all_results.push((idx, m));
                        }
                        iteration_matched = true;
                        break;
                    }
                    _ => {
                        self.stream.restore(saved);
                    }
                }
            }

            if !iteration_matched { break; }
        }

        Ok(all_results)
    }

    fn try_match_with_cardinality(
        &mut self,
        alt: &ArchivedAlternative,
    ) -> Result<Vec<Vec<ParseValue>>, ParseError> {
        match &alt.cardinality {
            ArchivedCardinality::One => {
                let values = self.try_match_items(&alt.items)?;
                Ok(vec![values])
            }
            ArchivedCardinality::ZeroOrMore => {
                let mut results = Vec::new();
                loop {
                    let saved = self.stream.save();
                    self.stream.skip_newlines();
                    match self.try_match_items(&alt.items) {
                        Ok(values) => results.push(values),
                        Err(_) => { self.stream.restore(saved); break; }
                    }
                }
                Ok(results)
            }
            ArchivedCardinality::OneOrMore => {
                let first = self.try_match_items(&alt.items)?;
                let mut results = vec![first];
                loop {
                    let saved = self.stream.save();
                    self.stream.skip_newlines();
                    match self.try_match_items(&alt.items) {
                        Ok(values) => results.push(values),
                        Err(_) => { self.stream.restore(saved); break; }
                    }
                }
                Ok(results)
            }
            ArchivedCardinality::Optional => {
                let saved = self.stream.save();
                match self.try_match_items(&alt.items) {
                    Ok(values) => Ok(vec![values]),
                    Err(_) => { self.stream.restore(saved); Ok(vec![]) }
                }
            }
        }
    }

    // ── Item matching ───────────────────────────────────────

    fn match_items(&mut self, items: &[ArchivedItem]) -> Result<Vec<ParseValue>, ParseError> {
        let mut values = Vec::new();
        for item in items.iter() {
            values.push(self.match_item(item)?);
        }
        Ok(values)
    }

    fn try_match_items(&mut self, items: &[ArchivedItem]) -> Result<Vec<ParseValue>, ParseError> {
        self.match_items(items)
    }

    fn match_item(&mut self, item: &ArchivedItem) -> Result<ParseValue, ParseError> {
        // Check adjacency
        if item.adjacent {
            self.check_adjacent()?;
        }

        match &item.content {
            ArchivedItemContent::Named { label } => self.match_named(label),
            ArchivedItemContent::DialectRef { target } => self.parse_dialect(target),
            ArchivedItemContent::Delimited { kind, inner } => self.match_delimited(kind, inner),
            ArchivedItemContent::Literal { token } => self.match_literal_token(token),
            ArchivedItemContent::Keyword { token } => self.match_keyword(token),
            ArchivedItemContent::Repeat { kind, inner } => self.match_repeat(kind, inner),
            ArchivedItemContent::LiteralValue => self.match_literal_value(),
        }
    }

    // ── Named (declare/reference) ───────────────────────────

    fn match_named(&mut self, label: &ArchivedLabel) -> Result<ParseValue, ParseError> {
        // Special case: LabelKind::Literal matches literal value tokens
        if matches!(label.kind, ArchivedLabelKind::Literal) {
            return self.match_literal_value();
        }

        let span = self.stream.span_here();
        match &label.casing {
            ArchivedCasing::Pascal => {
                let (name, span) = self.stream.expect_pascal()?;
                Ok(ParseValue::Name(name, span))
            }
            ArchivedCasing::Camel => {
                let (name, span) = self.stream.expect_camel()?;
                Ok(ParseValue::Name(name, span))
            }
        }
    }

    // ── Literal token ───────────────────────────────────────

    fn match_literal_token(&mut self, expected: &ArchivedLiteralToken) -> Result<ParseValue, ParseError> {
        let start = self.stream.span_here().start;

        match expected {
            ArchivedLiteralToken::At => { self.stream.expect(&Token::At)?; }
            ArchivedLiteralToken::MutAt => {
                self.stream.expect(&Token::Tilde)?;
                self.check_adjacent()?;
                self.stream.expect(&Token::At)?;
            }
            ArchivedLiteralToken::BorrowAt => {
                self.stream.expect(&Token::Colon)?;
                self.check_adjacent()?;
                self.stream.expect(&Token::At)?;
            }
            ArchivedLiteralToken::Dollar => { self.stream.expect(&Token::Dollar)?; }
            ArchivedLiteralToken::Star => { self.stream.expect(&Token::Star)?; }
            ArchivedLiteralToken::Plus => { self.stream.expect(&Token::Plus)?; }
            ArchivedLiteralToken::Question => { self.stream.expect(&Token::Question)?; }
            ArchivedLiteralToken::Ampersand => { self.stream.expect(&Token::Ampersand)?; }
            ArchivedLiteralToken::Caret => { self.stream.expect(&Token::Caret)?; }
            ArchivedLiteralToken::Dot => { self.stream.expect(&Token::Dot)?; }
            ArchivedLiteralToken::Slash => { self.stream.expect(&Token::Slash)?; }
            ArchivedLiteralToken::LogicalOr => { self.stream.expect(&Token::LogicalOr)?; }
            ArchivedLiteralToken::LogicalAnd => { self.stream.expect(&Token::LogicalAnd)?; }
            ArchivedLiteralToken::Eq => { self.stream.expect(&Token::DoubleEquals)?; }
            ArchivedLiteralToken::NotEq => { self.stream.expect(&Token::NotEqual)?; }
            ArchivedLiteralToken::Lt => { self.stream.expect(&Token::LessThan)?; }
            ArchivedLiteralToken::Gt => { self.stream.expect(&Token::GreaterThan)?; }
            ArchivedLiteralToken::LtEq => { self.stream.expect(&Token::LessThanOrEqual)?; }
            ArchivedLiteralToken::GtEq => { self.stream.expect(&Token::GreaterThanOrEqual)?; }
            ArchivedLiteralToken::Minus => { self.stream.expect(&Token::Minus)?; }
            ArchivedLiteralToken::Percent => { self.stream.expect(&Token::Percent)?; }
            ArchivedLiteralToken::InlineOr => { self.stream.expect(&Token::Pipe)?; }
            ArchivedLiteralToken::Colon => { self.stream.expect(&Token::Colon)?; }
            ArchivedLiteralToken::Pipe => { self.stream.expect(&Token::Pipe)?; }
        }

        let end = self.stream.prev_span().end;
        Ok(ParseValue::Token(Span { start, end }))
    }

    // ── Keyword ─────────────────────────────────────────────

    fn match_keyword(&mut self, expected: &ArchivedKeywordToken) -> Result<ParseValue, ParseError> {
        let span = self.stream.span_here();
        match (expected, self.stream.peek_token()) {
            (ArchivedKeywordToken::Self_, Some(Token::PascalIdent(s))) if s == "Self" => {
                self.stream.advance();
                Ok(ParseValue::Keyword(span))
            }
            (ArchivedKeywordToken::Main, Some(Token::PascalIdent(s))) if s == "Main" => {
                self.stream.advance();
                Ok(ParseValue::Keyword(span))
            }
            _ => Err(ParseError::new(
                format!("expected keyword {:?}", expected),
                span,
            )),
        }
    }

    // ── Literal value ───────────────────────────────────────

    fn match_literal_value(&mut self) -> Result<ParseValue, ParseError> {
        let span = self.stream.span_here();
        match self.stream.peek_token().cloned() {
            Some(Token::Integer(n)) => {
                self.stream.advance();
                Ok(ParseValue::Literal(sema_core::LiteralValue::Int(n), span))
            }
            Some(Token::Float(s)) => {
                self.stream.advance();
                Ok(ParseValue::Literal(
                    sema_core::LiteralValue::Float(s.parse::<f64>().unwrap_or(0.0)),
                    span,
                ))
            }
            Some(Token::StringLit(s)) => {
                self.stream.advance();
                Ok(ParseValue::Literal(sema_core::LiteralValue::Str(s), span))
            }
            _ => Err(ParseError::new("expected literal value".into(), span)),
        }
    }

    // ── Delimited ────────────────────────────────────────────

    fn match_delimited(
        &mut self,
        kind: &ArchivedDelimKind,
        inner: &[ArchivedItem],
    ) -> Result<ParseValue, ParseError> {
        let (open, close) = Self::delim_tokens(kind);
        self.stream.expect(&open)?;
        self.stream.skip_newlines();

        let mut values = Vec::new();
        for item in inner.iter() {
            self.stream.skip_newlines();
            values.push(self.match_item(item)?);
        }

        self.stream.skip_newlines();
        self.stream.expect(&close)?;
        Ok(ParseValue::Seq(values))
    }

    fn delim_tokens(kind: &ArchivedDelimKind) -> (Token, Token) {
        match kind {
            ArchivedDelimKind::Paren => (Token::LParen, Token::RParen),
            ArchivedDelimKind::Bracket => (Token::LBracket, Token::RBracket),
            ArchivedDelimKind::Brace => (Token::LBrace, Token::RBrace),
            ArchivedDelimKind::ParenPipe => (Token::LParenPipe, Token::RPipeParen),
            ArchivedDelimKind::BracketPipe => (Token::LBracketPipe, Token::RPipeBracket),
            ArchivedDelimKind::BracePipe => (Token::LBracePipe, Token::RPipeBrace),
        }
    }

    // ── Repeat ──────────────────────────────────────────────

    fn match_repeat(
        &mut self,
        kind: &ArchivedCardinality,
        inner: &ArchivedItem,
    ) -> Result<ParseValue, ParseError> {
        match kind {
            ArchivedCardinality::ZeroOrMore => {
                let mut results = Vec::new();
                loop {
                    let saved = self.stream.save();
                    match self.match_item(inner) {
                        Ok(v) => results.push(v),
                        Err(_) => { self.stream.restore(saved); break; }
                    }
                }
                Ok(ParseValue::Seq(results))
            }
            ArchivedCardinality::OneOrMore => {
                let first = self.match_item(inner)?;
                let mut results = vec![first];
                loop {
                    let saved = self.stream.save();
                    match self.match_item(inner) {
                        Ok(v) => results.push(v),
                        Err(_) => { self.stream.restore(saved); break; }
                    }
                }
                Ok(ParseValue::Seq(results))
            }
            ArchivedCardinality::Optional => {
                let saved = self.stream.save();
                match self.match_item(inner) {
                    Ok(v) => Ok(v),
                    Err(_) => { self.stream.restore(saved); Ok(ParseValue::None_) }
                }
            }
            ArchivedCardinality::One => self.match_item(inner),
        }
    }

    // ── Adjacency ───────────────────────────────────────────

    fn check_adjacent(&self) -> Result<(), ParseError> {
        if !self.stream.is_adjacent() {
            let span = self.stream.span_here();
            return Err(ParseError::new("expected adjacent tokens (no space)".into(), span));
        }
        Ok(())
    }

    // ── Error helpers ───────────────────────────────────────

    fn error(&self, message: &str) -> ParseError {
        let mut err = ParseError::new(message.to_string(), self.stream.span_here());
        for ctx in &self.context_stack {
            err = err.with_context(ctx.clone());
        }
        err
    }

    // ── Build from dialect (stub — implemented in build.rs) ─

    fn build_from_dialect(
        &self,
        _kind: &ArchivedDialectKind,
        _rules: Vec<MatchedRule>,
    ) -> Result<ParseValue, ParseError> {
        // TODO: implement in build.rs
        Err(self.error("build_from_dialect not yet implemented"))
    }

    fn build_postfix(
        &self,
        _kind: &ArchivedDialectKind,
        _alt_idx: usize,
        _base: Vec<ParseValue>,
        _postfix: Vec<ParseValue>,
    ) -> Result<Vec<ParseValue>, ParseError> {
        // TODO: implement in build.rs
        Err(self.error("build_postfix not yet implemented"))
    }
}
