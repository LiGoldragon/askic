/// Domain tree types — the typed parse tree.
///
/// Flat arena architecture. No recursive types — all references
/// are typed indices into arena vecs. This IS the sema-native
/// representation: domain variants as flat data, indices as refs.
///
/// The arena IS the .sema file. rkyv zero-copy mmap.

use rkyv::{Archive, Serialize, Deserialize};

// ── Typed arena indices ─────────────────────────────────────
// Each is a u32 index into the corresponding arena vec.

macro_rules! arena_ref {
    ($name:ident) => {
        #[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub u32);

        impl $name {
            pub fn idx(self) -> usize { self.0 as usize }
        }
    };
}

arena_ref!(ExprRef);
arena_ref!(StmtRef);
arena_ref!(BlockRef);
arena_ref!(MatchRef);
arena_ref!(FieldInitRef);
arena_ref!(EnumChildRef);
arena_ref!(StructChildRef);
arena_ref!(MethodDefRef);
arena_ref!(MatchArmRef);
arena_ref!(ParamRef);

// ── The Arena — the .sema data ──────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Default)]
pub struct Arena {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Statement>,
    pub blocks: Vec<Block>,
    pub matches: Vec<MatchExpr>,
    pub field_inits: Vec<FieldInit>,
    pub enum_children: Vec<EnumChild>,
    pub struct_children: Vec<StructChild>,
    pub method_defs: Vec<MethodDef>,
    pub match_arms: Vec<MatchArm>,
    pub params: Vec<Param>,
}

impl Arena {
    pub fn alloc_expr(&mut self, expr: Expr) -> ExprRef {
        let idx = self.exprs.len() as u32;
        self.exprs.push(expr);
        ExprRef(idx)
    }

    pub fn alloc_stmt(&mut self, stmt: Statement) -> StmtRef {
        let idx = self.stmts.len() as u32;
        self.stmts.push(stmt);
        StmtRef(idx)
    }

    pub fn alloc_block(&mut self, block: Block) -> BlockRef {
        let idx = self.blocks.len() as u32;
        self.blocks.push(block);
        BlockRef(idx)
    }

    pub fn alloc_match(&mut self, m: MatchExpr) -> MatchRef {
        let idx = self.matches.len() as u32;
        self.matches.push(m);
        MatchRef(idx)
    }

    pub fn alloc_field_init(&mut self, fi: FieldInit) -> FieldInitRef {
        let idx = self.field_inits.len() as u32;
        self.field_inits.push(fi);
        FieldInitRef(idx)
    }

    pub fn alloc_enum_child(&mut self, ec: EnumChild) -> EnumChildRef {
        let idx = self.enum_children.len() as u32;
        self.enum_children.push(ec);
        EnumChildRef(idx)
    }

    pub fn alloc_struct_child(&mut self, sc: StructChild) -> StructChildRef {
        let idx = self.struct_children.len() as u32;
        self.struct_children.push(sc);
        StructChildRef(idx)
    }

    pub fn alloc_method_def(&mut self, md: MethodDef) -> MethodDefRef {
        let idx = self.method_defs.len() as u32;
        self.method_defs.push(md);
        MethodDefRef(idx)
    }

    pub fn alloc_match_arm(&mut self, ma: MatchArm) -> MatchArmRef {
        let idx = self.match_arms.len() as u32;
        self.match_arms.push(ma);
        MatchArmRef(idx)
    }

    pub fn alloc_param(&mut self, p: Param) -> ParamRef {
        let idx = self.params.len() as u32;
        self.params.push(p);
        ParamRef(idx)
    }
}

// ── Classification types (from aski-core via cc) ────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameDomain {
    Type, Variant, Field, Trait, Method,
    Module, Literal, TypeParam,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Add, Sub, Mul, Mod,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility { Exported, Local }

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Span { pub start: u32, pub end: u32 }

// ── Name ────────────────────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name(pub String);

impl Name {
    pub fn new(s: &str) -> Self { Name(s.to_string()) }
}

// ── Type expressions ────────────────────────────────────────
// No recursion — Application args are a range into a separate vec
// or we keep Vec<TypeExpr> since TypeExpr doesn't recurse deeply.

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(serialize_bounds(__S: rkyv::ser::Writer + rkyv::ser::Allocator, __S::Error: rkyv::rancor::Source))]
#[rkyv(deserialize_bounds(__D::Error: rkyv::rancor::Source))]
pub enum TypeExpr {
    Simple(Name),
    Application { constructor: Name, #[rkyv(omit_bounds)] args: Vec<TypeExpr> },
    Param(Name),
    BoundedParam { bounds: Vec<Name> },
    InstanceRef { constructor: Name, #[rkyv(omit_bounds)] args: Vec<TypeExpr> },
}

// ── Root-level definitions ──────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum RootChild {
    Module(ModuleDef),
    Enum(EnumDef),
    Struct(StructDef),
    Newtype(NewtypeDef),
    TraitDecl(TraitDeclDef),
    TraitImpl(TraitImplDef),
    Const(ConstDef),
    Ffi(FfiDef),
    Process(BlockRef),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ModuleDef {
    pub name: Name,
    pub exports: Vec<Name>,
    pub imports: Vec<ModuleImport>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ModuleImport {
    pub source: Name,
    pub names: Vec<Name>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EnumDef {
    pub name: Name,
    pub children: Vec<EnumChildRef>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum EnumChild {
    Variant { name: Name, span: Span },
    DataVariant { name: Name, payload: TypeExpr, span: Span },
    StructVariant { name: Name, fields: Vec<StructField>, span: Span },
    NestedEnum(EnumDef),
    NestedStruct(StructDef),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct StructDef {
    pub name: Name,
    pub children: Vec<StructChildRef>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum StructChild {
    TypedField { name: Name, typ: TypeExpr, span: Span },
    SelfTypedField { name: Name, span: Span },
    NestedEnum(EnumDef),
    NestedStruct(StructDef),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct StructField {
    pub name: Name,
    pub typ: TypeExpr,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct NewtypeDef {
    pub name: Name,
    pub wraps: TypeExpr,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct ConstDef {
    pub name: Name,
    pub typ: TypeExpr,
    pub value: LiteralValue,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FfiDef {
    pub library: Name,
    pub functions: Vec<FfiFunction>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FfiFunction {
    pub name: Name,
    pub params: Vec<ParamRef>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

// ── Expressions ─────────────────────────────────────────────
// All sub-expression references are ExprRef (arena index).
// No Box, no recursion. Flat.

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum Expr {
    BinAdd { left: ExprRef, right: ExprRef, span: Span },
    BinSub { left: ExprRef, right: ExprRef, span: Span },
    BinMul { left: ExprRef, right: ExprRef, span: Span },
    BinMod { left: ExprRef, right: ExprRef, span: Span },
    BinEq { left: ExprRef, right: ExprRef, span: Span },
    BinNotEq { left: ExprRef, right: ExprRef, span: Span },
    BinLt { left: ExprRef, right: ExprRef, span: Span },
    BinGt { left: ExprRef, right: ExprRef, span: Span },
    BinLtEq { left: ExprRef, right: ExprRef, span: Span },
    BinGtEq { left: ExprRef, right: ExprRef, span: Span },
    BinAnd { left: ExprRef, right: ExprRef, span: Span },
    BinOr { left: ExprRef, right: ExprRef, span: Span },
    FieldAccess { object: ExprRef, field: Name, span: Span },
    MethodCall { object: ExprRef, method: Name, args: Vec<ExprRef>, span: Span },
    TryUnwrap { inner: ExprRef, span: Span },
    InstanceRef { name: Name, span: Span },
    PathVariant { typ: Name, variant: Name, span: Span },
    PathMethod { typ: Name, method: Name, args: Vec<ExprRef>, span: Span },
    IntLit { value: i64, span: Span },
    FloatLit { value: f64, span: Span },
    StringLit { value: String, span: Span },
    InlineEval(BlockRef),
    Match(MatchRef),
    Loop(BlockRef),
    Iteration { source: ExprRef, body: BlockRef },
    StructConstruct { typ: Name, fields: Vec<FieldInitRef>, span: Span },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FieldInit {
    pub name: Name,
    pub value: ExprRef,
}

// ── Statements ──────────────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum Statement {
    EarlyReturn(ExprRef),
    Loop(BlockRef),
    Iteration { source: ExprRef, body: BlockRef },
    LocalTypeDecl { name: Name, typ: TypeExpr, span: Span },
    Mutation { name: Name, method: Name, args: Vec<ExprRef>, span: Span },
    Instance { name: Name, type_annotation: Option<TypeExpr>, value: ExprRef, span: Span },
    Expr(ExprRef),
}

// ── Bodies ──────────────────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub stmts: Vec<StmtRef>,
    pub tail: Option<ExprRef>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum MethodBody {
    Block(BlockRef),
    Match(MatchRef),
    Loop(BlockRef),
    Iteration { source: ExprRef, body: BlockRef },
    StructConstruct { typ: Name, fields: Vec<FieldInitRef>, span: Span },
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum Param {
    BorrowSelf,
    MutBorrowSelf,
    OwnedSelf,
    Named { name: Name, typ: TypeExpr },
    Bare { name: Name },
}

// ── Match ───────────────────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct MatchExpr {
    pub target: Option<ExprRef>,
    pub arms: Vec<MatchArmRef>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct MatchArm {
    pub patterns: Vec<Pattern>,
    pub result: ExprRef,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum Pattern {
    Variant(Name),
    VariantBind { variant: Name, binding: Name },
    OrPattern(Vec<Name>),
    StringLit(String),
}

// ── Traits ──────────────────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TraitDeclDef {
    pub name: Name,
    pub signatures: Vec<MethodSig>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct MethodSig {
    pub name: Name,
    pub params: Vec<ParamRef>,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct TraitImplDef {
    pub trait_name: Name,
    pub type_name: Name,
    pub methods: Vec<MethodDefRef>,
    pub span: Span,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct MethodDef {
    pub name: Name,
    pub params: Vec<ParamRef>,
    pub return_type: Option<TypeExpr>,
    pub body: MethodBody,
    pub span: Span,
}

// ── The top-level output ────────────────────────────────────

#[derive(Archive, Serialize, Deserialize, Debug, Clone, Default)]
pub struct AskiProgram {
    pub children: Vec<RootChild>,
    pub arena: Arena,
}
