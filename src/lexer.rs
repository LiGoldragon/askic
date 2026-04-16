use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq, Eq, Hash)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // === Comments ===
    #[regex(r";;[^\n]*")]
    Comment,

    // === Newlines (significant for statement separation) ===
    #[regex(r"\n+")]
    Newline,

    // === Multi-char operators (must come before single-char) ===
    #[token("(|")]
    LParenPipe,

    #[token("|)")]
    RPipeParen,

    #[token("{|")]
    LBracePipe,

    #[token("|}")]
    RPipeBrace,

    #[token("[|")]
    LBracketPipe,

    #[token("|]")]
    RPipeBracket,


    // === Delimiters ===
    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    // === Symbols ===
    #[token(".")]
    Dot,

    #[token("@")]
    At,

    #[token("$")]
    Dollar,

    #[token("^")]
    Caret,

    #[token(">")]
    GreaterThan,

    #[token("<")]
    LessThan,

    #[token("&")]
    Ampersand,

    #[token("~")]
    Tilde,

    #[token("?")]
    Question,


    #[token("|")]
    Pipe,


    #[token(":")]
    Colon,

    // === Arithmetic / comparison operators ===
    #[token("+")]
    Plus,

    #[token("-")]
    Minus,

    #[token("*")]
    Star,

    #[token("/")]
    Slash,

    #[token("%")]
    Percent,

    #[token("==")]
    DoubleEquals,


    #[token("!=")]
    NotEqual,

    #[token(">=")]
    GreaterThanOrEqual,

    #[token("<=")]
    LessThanOrEqual,

    #[token("&&")]
    LogicalAnd,

    #[token("||")]
    LogicalOr,



    // === PascalCase identifier (starts with uppercase) ===
    #[regex(r"[A-Z][a-zA-Z0-9]*", |lex| lex.slice().to_string())]
    PascalIdent(String),

    // === camelCase identifier (starts with lowercase, may contain underscores) ===
    #[regex(r"[a-z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    CamelIdent(String),

    // === Numeric literals ===
    #[regex(r"[0-9]+\.[0-9]+", |lex| Some(lex.slice().to_string()))]
    Float(String),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok(), priority = 2)]
    Integer(i64),

    // === String literals (supports \" \n \\ escapes) ===
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        let mut out = String::new();
        let mut escape = false;
        for ch in inner.chars() {
            if escape {
                match ch {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    other => { out.push('\\'); out.push(other); }
                }
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else {
                out.push(ch);
            }
        }
        out
    })]
    StringLit(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Comment => write!(f, ";;"),
            Token::Newline => write!(f, "\\n"),
            Token::LBracePipe => write!(f, "{{|"),
            Token::RPipeBrace => write!(f, "|}}"),
            Token::LParenPipe => write!(f, "(|"),
            Token::RPipeParen => write!(f, "|)"),
            Token::LBracketPipe => write!(f, "[|"),
            Token::RPipeBracket => write!(f, "|]"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Dot => write!(f, "."),
            Token::At => write!(f, "@"),
            Token::Dollar => write!(f, "$"),
            Token::Caret => write!(f, "^"),
            Token::GreaterThan => write!(f, ">"),
            Token::LessThan => write!(f, "<"),
            Token::Ampersand => write!(f, "&"),
            Token::Tilde => write!(f, "~"),
            Token::Question => write!(f, "?"),
            Token::Pipe => write!(f, "|"),
            Token::Colon => write!(f, ":"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::DoubleEquals => write!(f, "=="),
            Token::NotEqual => write!(f, "!="),
            Token::GreaterThanOrEqual => write!(f, ">="),
            Token::LessThanOrEqual => write!(f, "<="),
            Token::LogicalAnd => write!(f, "&&"),
            Token::LogicalOr => write!(f, "||"),
            Token::PascalIdent(s) => write!(f, "{s}"),
            Token::CamelIdent(s) => write!(f, "{s}"),
            Token::Float(v) => write!(f, "{}", v),
            Token::Integer(v) => write!(f, "{v}"),
            Token::StringLit(s) => write!(f, "\"{s}\""),
        }
    }
}

/// A token with its span in the source text.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// Tokenize aski source into a list of spanned tokens, filtering comments.
pub fn lex(source: &str) -> Result<Vec<Spanned>, Vec<LexError>> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(token) => {
                // Skip comments — they carry no semantic content
                if !matches!(token, Token::Comment) {
                    tokens.push(Spanned { token, span });
                }
            }
            Err(()) => {
                errors.push(LexError {
                    span: span.clone(),
                    text: source[span].to_string(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub span: std::ops::Range<usize>,
    pub text: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unexpected character '{}' at {}..{}",
            self.text, self.span.start, self.span.end
        )
    }
}

