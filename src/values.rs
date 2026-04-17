/// Intermediate values produced during dialect matching.

use sema_core::*;

/// What a single item match produces.
#[derive(Debug)]
pub enum ParseValue {
    /// A declared or referenced name.
    Name(String, Span),
    /// A literal value (int, float, string, bool).
    Literal(LiteralValue, Span),
    /// A keyword was matched (Self, Main).
    Keyword(Span),
    /// A matched literal/operator token — no extracted value.
    Token(Span),
    /// A dialect produced a typed value.
    Dialect(DialectValue),
    /// A sequence of repeated matches.
    Seq(Vec<ParseValue>),
    /// Optional match that didn't match.
    None_,
}

/// What a dialect produces — typed sema-core values.
#[derive(Debug)]
pub enum DialectValue {
    RootChildren(Vec<RootChild>),
    Module(ModuleDef),
    EnumChildren(Vec<EnumChild>),
    StructChildren(Vec<StructChild>),
    Expr(Expr),
    TypeExpr(TypeExpr),
    TypeApp(TypeApplication),
    Statement(Statement),
    Block(Block),
    Pattern(Pattern),
    MatchExpr(MatchExpr),
    LoopExpr(LoopExpr),
    Iteration(Iteration),
    Param(Param),
    Params(Vec<Param>),
    MethodSig(MethodSig),
    MethodDef(MethodDef),
    MethodBody(MethodBody),
    TraitDecl(TraitDeclDef),
    TraitImpl(TraitImplDef),
    Instance(Instance),
    Mutation(Mutation),
    IterationSource { source: Expr, binding: Pattern },
    StructConstruct { typ: TypeName, fields: Vec<FieldInit> },
    FfiDef(FfiDef),
    Methods(Vec<MethodDef>),
    GenericParamDef(GenericParamDef),
    Signatures(Vec<MethodSig>),
}

/// What a rule match produces.
#[derive(Debug)]
pub enum MatchedRule {
    Sequential(Vec<ParseValue>),
    Choice(usize, Vec<ParseValue>),
    RepeatedChoice(Vec<(usize, Vec<ParseValue>)>),
}

impl ParseValue {
    pub fn as_name(&self) -> String {
        match self {
            ParseValue::Name(s, _) => s.clone(),
            other => panic!("expected Name, got {:?}", other),
        }
    }

    pub fn as_span(&self) -> Span {
        match self {
            ParseValue::Name(_, s) => s.clone(),
            ParseValue::Literal(_, s) => s.clone(),
            ParseValue::Keyword(s) => s.clone(),
            ParseValue::Token(s) => s.clone(),
            _ => Span { start: 0, end: 0 },
        }
    }

    pub fn as_expr(&self) -> Expr {
        match self {
            ParseValue::Dialect(DialectValue::Expr(e)) => e.clone(),
            other => panic!("expected Expr, got {:?}", other),
        }
    }

    pub fn as_type_expr(&self) -> TypeExpr {
        match self {
            ParseValue::Dialect(DialectValue::TypeExpr(t)) => t.clone(),
            // Unwrap Seq from optional/repeat wrapping
            ParseValue::Seq(v) if v.len() == 1 => v[0].as_type_expr(),
            other => panic!("expected TypeExpr, got {:?}", other),
        }
    }

    pub fn as_block(&self) -> Block {
        match self {
            ParseValue::Dialect(DialectValue::Block(b)) => b.clone(),
            other => panic!("expected Block, got {:?}", other),
        }
    }

    pub fn as_pattern(&self) -> Pattern {
        match self {
            ParseValue::Dialect(DialectValue::Pattern(p)) => p.clone(),
            other => panic!("expected Pattern, got {:?}", other),
        }
    }

    pub fn as_match_expr(&self) -> MatchExpr {
        match self {
            ParseValue::Dialect(DialectValue::MatchExpr(m)) => m.clone(),
            other => panic!("expected MatchExpr, got {:?}", other),
        }
    }

    pub fn as_loop_expr(&self) -> LoopExpr {
        match self {
            ParseValue::Dialect(DialectValue::LoopExpr(l)) => l.clone(),
            other => panic!("expected LoopExpr, got {:?}", other),
        }
    }

    pub fn as_iteration(&self) -> Iteration {
        match self {
            ParseValue::Dialect(DialectValue::Iteration(i)) => i.clone(),
            other => panic!("expected Iteration, got {:?}", other),
        }
    }

    pub fn as_method_body(&self) -> MethodBody {
        match self {
            ParseValue::Dialect(DialectValue::MethodBody(m)) => m.clone(),
            other => panic!("expected MethodBody, got {:?}", other),
        }
    }

    pub fn as_param(&self) -> Param {
        match self {
            ParseValue::Dialect(DialectValue::Param(p)) => p.clone(),
            other => panic!("expected Param, got {:?}", other),
        }
    }

    pub fn as_literal(&self) -> LiteralValue {
        match self {
            ParseValue::Literal(v, _) => v.clone(),
            other => panic!("expected LiteralValue, got {:?}", other),
        }
    }

    pub fn as_seq(&self) -> &[ParseValue] {
        match self {
            ParseValue::Seq(v) => v,
            other => panic!("expected Seq, got {:?}", other),
        }
    }

    pub fn as_statement(&self) -> Statement {
        match self {
            ParseValue::Dialect(DialectValue::Statement(s)) => s.clone(),
            other => panic!("expected Statement, got {:?}", other),
        }
    }

    pub fn as_mutation(&self) -> Mutation {
        match self {
            ParseValue::Dialect(DialectValue::Mutation(m)) => m.clone(),
            other => panic!("expected Mutation, got {:?}", other),
        }
    }

    pub fn as_instance(&self) -> Instance {
        match self {
            ParseValue::Dialect(DialectValue::Instance(i)) => i.clone(),
            other => panic!("expected Instance, got {:?}", other),
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, ParseValue::None_)
    }
}
