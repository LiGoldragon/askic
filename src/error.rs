/// Parse error — position, message, context stack.

use sema_core::Span;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub context: Vec<String>,
}

impl ParseError {
    pub fn new(message: String, span: Span) -> Self {
        ParseError { message, span, context: Vec::new() }
    }

    pub fn with_context(mut self, ctx: String) -> Self {
        self.context.push(ctx);
        self
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error at {}..{}: {}", self.span.start, self.span.end, self.message)?;
        for ctx in self.context.iter().rev() {
            write!(f, "\n  in {}", ctx)?;
        }
        Ok(())
    }
}
