/// Typed engine output. Every variant carries the type
/// the dialect data says that position produces.
///
/// No Seq. No untyped bags. No as_* panic methods.

use aski_core::*;

/// What any item match or dialect entry produces.
pub enum Typed {
    // ── Leaf tokens ────────────────────────────────
    PascalName(String, Span),
    CamelName(String, Span),
    Literal(LiteralValue, Span),
    Token(Span),
    Keyword(Span),
    None_,

    // ── Domain types (1:1 with DialectKind) ────────
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
    MethodSig(MethodSig),
    MethodDef(MethodDef),
    TraitDecl(TraitDeclDef),
    TraitImpl(TraitImplDef),
    Instance(Instance),
    Mutation(Mutation),
    IterSource { source: Expr, binding: Pattern },
    StructConstruct { typ: TypeName, fields: Vec<FieldInit> },
    FfiDef(FfiDef),
    Methods(Vec<MethodDef>),

    // ── Typed collections (replace Seq) ────────────
    Exprs(Vec<Expr>),
    TypeExprs(Vec<TypeExpr>),
    Statements(Vec<Statement>),
    Params(Vec<Param>),
    Patterns(Vec<Pattern>),
    MethodSigs(Vec<MethodSig>),
    MethodDefs(Vec<MethodDef>),
    MatchArms(Vec<MatchArm>),
    PascalNames(Vec<(String, Span)>),
    CamelNames(Vec<(String, Span)>),
    FieldInits(Vec<FieldInit>),
    FfiFunctions(Vec<FfiFunction>),
    ExportItems(Vec<ExportItem>),
    ImportBlocks(Vec<ModuleImport>),

    // ── Positional group (immediately destructured) ─
    Group(ItemTuple),
}

/// Fixed-arity positional tuple from delimited groups.
/// Always immediately destructured. Never stored.
pub struct ItemTuple(pub Vec<Typed>);

impl ItemTuple {
    pub fn take(&mut self, idx: usize) -> Typed {
        std::mem::replace(&mut self.0[idx], Typed::None_)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Result-returning extractors. Never panic.
impl Typed {
    pub fn into_module(self) -> Result<ModuleDef, String> {
        match self { Typed::Module(m) => Ok(m), other => Err(format!("expected Module, got {}", other.tag())) }
    }

    pub fn into_expr(self) -> Result<Expr, String> {
        match self { Typed::Expr(e) => Ok(e), other => Err(format!("expected Expr, got {}", other.tag())) }
    }

    pub fn into_type_expr(self) -> Result<TypeExpr, String> {
        match self { Typed::TypeExpr(t) => Ok(t), other => Err(format!("expected TypeExpr, got {}", other.tag())) }
    }

    pub fn into_type_app(self) -> Result<TypeApplication, String> {
        match self { Typed::TypeApp(t) => Ok(t), other => Err(format!("expected TypeApp, got {}", other.tag())) }
    }

    pub fn into_block(self) -> Result<Block, String> {
        match self { Typed::Block(b) => Ok(b), other => Err(format!("expected Block, got {}", other.tag())) }
    }

    pub fn into_statement(self) -> Result<Statement, String> {
        match self { Typed::Statement(s) => Ok(s), other => Err(format!("expected Statement, got {}", other.tag())) }
    }

    pub fn into_pattern(self) -> Result<Pattern, String> {
        match self { Typed::Pattern(p) => Ok(p), other => Err(format!("expected Pattern, got {}", other.tag())) }
    }

    pub fn into_match_expr(self) -> Result<MatchExpr, String> {
        match self { Typed::MatchExpr(m) => Ok(m), other => Err(format!("expected MatchExpr, got {}", other.tag())) }
    }

    pub fn into_loop_expr(self) -> Result<LoopExpr, String> {
        match self { Typed::LoopExpr(l) => Ok(l), other => Err(format!("expected LoopExpr, got {}", other.tag())) }
    }

    pub fn into_iteration(self) -> Result<Iteration, String> {
        match self { Typed::Iteration(i) => Ok(i), other => Err(format!("expected Iteration, got {}", other.tag())) }
    }

    pub fn into_param(self) -> Result<Param, String> {
        match self { Typed::Param(p) => Ok(p), other => Err(format!("expected Param, got {}", other.tag())) }
    }

    pub fn into_method_sig(self) -> Result<MethodSig, String> {
        match self { Typed::MethodSig(s) => Ok(s), other => Err(format!("expected MethodSig, got {}", other.tag())) }
    }

    pub fn into_method_def(self) -> Result<MethodDef, String> {
        match self { Typed::MethodDef(m) => Ok(m), other => Err(format!("expected MethodDef, got {}", other.tag())) }
    }

    pub fn into_trait_decl(self) -> Result<TraitDeclDef, String> {
        match self { Typed::TraitDecl(t) => Ok(t), other => Err(format!("expected TraitDecl, got {}", other.tag())) }
    }

    pub fn into_trait_impl(self) -> Result<TraitImplDef, String> {
        match self { Typed::TraitImpl(t) => Ok(t), other => Err(format!("expected TraitImpl, got {}", other.tag())) }
    }

    pub fn into_instance(self) -> Result<Instance, String> {
        match self { Typed::Instance(i) => Ok(i), other => Err(format!("expected Instance, got {}", other.tag())) }
    }

    pub fn into_mutation(self) -> Result<Mutation, String> {
        match self { Typed::Mutation(m) => Ok(m), other => Err(format!("expected Mutation, got {}", other.tag())) }
    }

    pub fn into_ffi_def(self) -> Result<FfiDef, String> {
        match self { Typed::FfiDef(f) => Ok(f), other => Err(format!("expected FfiDef, got {}", other.tag())) }
    }

    pub fn into_enum_children(self) -> Result<Vec<EnumChild>, String> {
        match self { Typed::EnumChildren(c) => Ok(c), other => Err(format!("expected EnumChildren, got {}", other.tag())) }
    }

    pub fn into_struct_children(self) -> Result<Vec<StructChild>, String> {
        match self { Typed::StructChildren(c) => Ok(c), other => Err(format!("expected StructChildren, got {}", other.tag())) }
    }

    pub fn into_methods(self) -> Result<Vec<MethodDef>, String> {
        match self { Typed::Methods(m) => Ok(m), other => Err(format!("expected Methods, got {}", other.tag())) }
    }

    pub fn into_pascal_name(self) -> Result<(String, Span), String> {
        match self { Typed::PascalName(s, sp) => Ok((s, sp)), other => Err(format!("expected PascalName, got {}", other.tag())) }
    }

    pub fn into_camel_name(self) -> Result<(String, Span), String> {
        match self { Typed::CamelName(s, sp) => Ok((s, sp)), other => Err(format!("expected CamelName, got {}", other.tag())) }
    }

    pub fn into_literal(self) -> Result<(LiteralValue, Span), String> {
        match self { Typed::Literal(v, sp) => Ok((v, sp)), other => Err(format!("expected Literal, got {}", other.tag())) }
    }

    pub fn into_exprs(self) -> Result<Vec<Expr>, String> {
        match self { Typed::Exprs(v) => Ok(v), other => Err(format!("expected Exprs, got {}", other.tag())) }
    }

    pub fn into_statements(self) -> Result<Vec<Statement>, String> {
        match self { Typed::Statements(v) => Ok(v), other => Err(format!("expected Statements, got {}", other.tag())) }
    }

    pub fn into_params(self) -> Result<Vec<Param>, String> {
        match self { Typed::Params(v) => Ok(v), other => Err(format!("expected Params, got {}", other.tag())) }
    }

    pub fn into_type_exprs(self) -> Result<Vec<TypeExpr>, String> {
        match self { Typed::TypeExprs(v) => Ok(v), other => Err(format!("expected TypeExprs, got {}", other.tag())) }
    }

    pub fn into_pascal_names(self) -> Result<Vec<(String, Span)>, String> {
        match self { Typed::PascalNames(v) => Ok(v), other => Err(format!("expected PascalNames, got {}", other.tag())) }
    }

    pub fn into_camel_names(self) -> Result<Vec<(String, Span)>, String> {
        match self { Typed::CamelNames(v) => Ok(v), other => Err(format!("expected CamelNames, got {}", other.tag())) }
    }

    pub fn into_group(self) -> Result<ItemTuple, String> {
        match self { Typed::Group(g) => Ok(g), other => Err(format!("expected Group, got {}", other.tag())) }
    }

    pub fn into_iter_source(self) -> Result<(Expr, Pattern), String> {
        match self { Typed::IterSource { source, binding } => Ok((source, binding)), other => Err(format!("expected IterSource, got {}", other.tag())) }
    }

    pub fn into_struct_construct(self) -> Result<(TypeName, Vec<FieldInit>), String> {
        match self { Typed::StructConstruct { typ, fields } => Ok((typ, fields)), other => Err(format!("expected StructConstruct, got {}", other.tag())) }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Typed::PascalName(..) => "PascalName",
            Typed::CamelName(..) => "CamelName",
            Typed::Literal(..) => "Literal",
            Typed::Token(..) => "Token",
            Typed::Keyword(..) => "Keyword",
            Typed::None_ => "None",
            Typed::Module(..) => "Module",
            Typed::EnumChildren(..) => "EnumChildren",
            Typed::StructChildren(..) => "StructChildren",
            Typed::Expr(..) => "Expr",
            Typed::TypeExpr(..) => "TypeExpr",
            Typed::TypeApp(..) => "TypeApp",
            Typed::Statement(..) => "Statement",
            Typed::Block(..) => "Block",
            Typed::Pattern(..) => "Pattern",
            Typed::MatchExpr(..) => "MatchExpr",
            Typed::LoopExpr(..) => "LoopExpr",
            Typed::Iteration(..) => "Iteration",
            Typed::Param(..) => "Param",
            Typed::MethodSig(..) => "MethodSig",
            Typed::MethodDef(..) => "MethodDef",
            Typed::TraitDecl(..) => "TraitDecl",
            Typed::TraitImpl(..) => "TraitImpl",
            Typed::Instance(..) => "Instance",
            Typed::Mutation(..) => "Mutation",
            Typed::IterSource { .. } => "IterSource",
            Typed::StructConstruct { .. } => "StructConstruct",
            Typed::FfiDef(..) => "FfiDef",
            Typed::Methods(..) => "Methods",
            Typed::Exprs(..) => "Exprs",
            Typed::TypeExprs(..) => "TypeExprs",
            Typed::Statements(..) => "Statements",
            Typed::Params(..) => "Params",
            Typed::Patterns(..) => "Patterns",
            Typed::MethodSigs(..) => "MethodSigs",
            Typed::MethodDefs(..) => "MethodDefs",
            Typed::MatchArms(..) => "MatchArms",
            Typed::PascalNames(..) => "PascalNames",
            Typed::CamelNames(..) => "CamelNames",
            Typed::FieldInits(..) => "FieldInits",
            Typed::FfiFunctions(..) => "FfiFunctions",
            Typed::ExportItems(..) => "ExportItems",
            Typed::ImportBlocks(..) => "ImportBlocks",
            Typed::Group(..) => "Group",
        }
    }
}
