/// Token stream — cursor over lexed tokens with save/restore.

use crate::lexer::{Token, Spanned};
use crate::error::ParseError;
use sema_core::Span;

pub struct TokenStream {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl TokenStream {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        TokenStream { tokens, pos: 0 }
    }

    pub fn peek(&self) -> Option<&Spanned> {
        self.tokens.get(self.pos)
    }

    pub fn peek_token(&self) -> Option<&Token> {
        self.peek().map(|s| &s.token)
    }

    pub fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    pub fn save(&self) -> usize {
        self.pos
    }

    pub fn restore(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub fn span_here(&self) -> Span {
        if let Some(s) = self.peek() {
            Span { start: s.span.start as u32, end: s.span.end as u32 }
        } else {
            let end = self.tokens.last()
                .map(|s| s.span.end as u32)
                .unwrap_or(0);
            Span { start: end, end }
        }
    }

    pub fn prev_span(&self) -> Span {
        if self.pos > 0 {
            let s = &self.tokens[self.pos - 1];
            Span { start: s.span.start as u32, end: s.span.end as u32 }
        } else {
            Span { start: 0, end: 0 }
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

    pub fn skip_newlines(&mut self) {
        while let Some(Token::Newline) = self.peek_token() {
            self.advance();
        }
    }

    pub fn expect(&mut self, expected: &Token) -> Result<Span, ParseError> {
        let span = self.span_here();
        match self.peek_token() {
            Some(t) if t == expected => {
                self.advance();
                Ok(span)
            }
            other => Err(ParseError::new(
                format!("expected {}, got {:?}", expected, other),
                span,
            )),
        }
    }

    pub fn expect_pascal(&mut self) -> Result<(String, Span), ParseError> {
        let span = self.span_here();
        match self.peek_token().cloned() {
            Some(Token::PascalIdent(s)) => {
                self.advance();
                Ok((s, span))
            }
            other => Err(ParseError::new(
                format!("expected PascalCase, got {:?}", other),
                span,
            )),
        }
    }

    pub fn expect_camel(&mut self) -> Result<(String, Span), ParseError> {
        let span = self.span_here();
        match self.peek_token().cloned() {
            Some(Token::CamelIdent(s)) => {
                self.advance();
                Ok((s, span))
            }
            other => Err(ParseError::new(
                format!("expected camelCase, got {:?}", other),
                span,
            )),
        }
    }
}
