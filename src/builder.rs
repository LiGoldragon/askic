/// Builder — per-dialect sema-core type constructors.
///
/// Each method takes ParseValues from the engine and
/// constructs sema-core types. All sema-core knowledge
/// lives here — the engine is generic.

use synth_core::*;
use aski_core::*;
use crate::values::*;

pub struct Builder;

impl Builder {
    pub fn new() -> Self {
        Builder
    }

    /// Dispatch to per-dialect builder based on DialectKind.
    pub fn build(
        &self,
        kind: &ArchivedDialectKind,
        rules: Vec<MatchedRule>,
    ) -> Result<ParseValue, String> {
        match kind {
            ArchivedDialectKind::Root => self.build_root(rules),
            ArchivedDialectKind::Module => self.build_module(rules),
            ArchivedDialectKind::Enum => self.build_enum(rules),
            ArchivedDialectKind::Struct => self.build_struct(rules),
            ArchivedDialectKind::Body => self.build_body(rules),
            ArchivedDialectKind::ExprOr => self.build_expr_binary(rules, BinOp::Or),
            ArchivedDialectKind::ExprAnd => self.build_expr_binary(rules, BinOp::And),
            ArchivedDialectKind::ExprCompare => self.build_expr_compare(rules),
            ArchivedDialectKind::ExprAdd => self.build_expr_add(rules),
            ArchivedDialectKind::ExprMul => self.build_expr_mul(rules),
            ArchivedDialectKind::ExprAtom => self.build_expr_atom(rules),
            ArchivedDialectKind::Type_ => self.build_type(rules),
            ArchivedDialectKind::TypeApplication => self.build_type_application(rules),
            ArchivedDialectKind::GenericParam => self.build_generic_param(rules),
            ArchivedDialectKind::Statement => self.build_statement(rules),
            ArchivedDialectKind::Instance => self.build_instance(rules),
            ArchivedDialectKind::Mutation => self.build_mutation(rules),
            ArchivedDialectKind::Param => self.build_param(rules),
            ArchivedDialectKind::Signature => self.build_signature(rules),
            ArchivedDialectKind::Method => self.build_method(rules),
            ArchivedDialectKind::TraitDecl => self.build_trait_decl(rules),
            ArchivedDialectKind::TraitImpl => self.build_trait_impl(rules),
            ArchivedDialectKind::TypeImpl => self.build_type_impl(rules),
            ArchivedDialectKind::Match => self.build_match(rules),
            ArchivedDialectKind::Pattern => self.build_pattern(rules),
            ArchivedDialectKind::Loop => self.build_loop(rules),
            ArchivedDialectKind::Process => self.build_process(rules),
            ArchivedDialectKind::IterationSource => self.build_iteration_source(rules),
            ArchivedDialectKind::StructConstruct => self.build_struct_construct(rules),
            ArchivedDialectKind::Ffi => self.build_ffi(rules),
            _ => Err("no builder for dialect".into()),
        }
    }

    /// Build postfix expression (field access, method call, try-unwrap).
    pub fn build_postfix(
        &self,
        alt_idx: usize,
        base: ParseValue,
        postfix: Vec<ParseValue>,
    ) -> Result<ParseValue, String> {
        let base_expr = base.as_expr();
        let start = base.as_span().start;

        let expr = match alt_idx {
            0 => {
                // .:Field → FieldAccess
                // postfix: [Token(dot), Name(field)]
                let field = FieldName(postfix[1].as_name());
                let end = postfix[1].as_span().end;
                Expr::FieldAccess {
                    object: Box::new(base_expr), field,
                    span: Span { start, end },
                }
            }
            1 => {
                // .:method(+<Expr>) → MethodCall
                // postfix: [Token(dot), Name(method), Seq(args)]
                let method = MethodName(postfix[1].as_name());
                let args: Vec<Expr> = match &postfix[2] {
                    ParseValue::Seq(v) => v.iter().map(|a| a.as_expr()).collect(),
                    _ => vec![],
                };
                let end = postfix.last().unwrap().as_span().end;
                Expr::MethodCall {
                    object: Box::new(base_expr), method, args,
                    span: Span { start, end },
                }
            }
            2 => {
                // _?_ → TryUnwrap
                let end = postfix[0].as_span().end;
                Expr::TryUnwrap {
                    inner: Box::new(base_expr),
                    span: Span { start, end },
                }
            }
            _ => return Err("unknown postfix alt".into()),
        };

        Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
    }

    // ── Root ────────────────────────────────────────────────

    fn build_root(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Rule 0: Sequential (@Module <Module>) — the module declaration
        // Rule 1: RepeatedChoice — all other root constructs
        //
        // The builder restructures the flat parse into a ModuleDef container.
        // Text is flat; the tree comes from here.

        // Module from rule 0
        let mut module = if let Some(MatchedRule::Sequential(ref items)) = rules.get(0) {
            let inner = items[0].as_seq();
            let name = TypeName(inner[0].as_name());
            match &inner[1] {
                ParseValue::Dialect(DialectValue::Module(m)) => {
                    let mut m = m.clone();
                    m.name = name;
                    m.span = Span {
                        start: inner[0].as_span().start,
                        end: inner[1].as_span().end,
                    };
                    m
                }
                _ => ModuleDef {
                    name,
                    visibility: Visibility::Public,
                    exports: vec![],
                    imports: vec![],
                    enums: vec![],
                    structs: vec![],
                    newtypes: vec![],
                    consts: vec![],
                    trait_decls: vec![],
                    trait_impls: vec![],
                    ffi: vec![],
                    process: None,
                    span: inner[0].as_span(),
                },
            }
        } else {
            return Err("missing module declaration".into());
        };

        // Other constructs from rule 1 — populate ModuleDef fields
        if let Some(MatchedRule::RepeatedChoice(ref repeated)) = rules.get(1) {
            for (alt_idx, values) in repeated {
                match alt_idx {
                    0 => {
                        // *(@Enum <Enum>)
                        let inner = values[0].as_seq();
                        let name = TypeName(inner[0].as_name());
                        let children = match &inner[1] {
                            ParseValue::Dialect(DialectValue::EnumChildren(c)) => c.clone(),
                            _ => return Err("expected enum children".into()),
                        };
                        module.enums.push(EnumDef {
                            name, visibility: Visibility::Public,
                            generic_params: vec![], derives: vec![],
                            children, span: Self::span_from_values(inner),
                        });
                    }
                    1 => {
                        // *(@trait <TraitDecl>)
                        let inner = values[0].as_seq();
                        let name = TraitName(inner[0].as_name());
                        match &inner[1] {
                            ParseValue::Dialect(DialectValue::TraitDecl(td)) => {
                                let mut td = td.clone();
                                td.name = name;
                                td.span = Self::span_from_values(inner);
                                module.trait_decls.push(td);
                            }
                            _ => return Err("expected trait decl".into()),
                        }
                    }
                    2 => {
                        // *[@trait <TraitImpl>]
                        let inner = values[0].as_seq();
                        let name = TraitName(inner[0].as_name());
                        match &inner[1] {
                            ParseValue::Dialect(DialectValue::TraitImpl(ti)) => {
                                let mut ti = ti.clone();
                                ti.trait_name = name;
                                ti.span = Self::span_from_values(inner);
                                module.trait_impls.push(ti);
                            }
                            _ => return Err("expected trait impl".into()),
                        }
                    }
                    3 => {
                        // *{@Struct <Struct>}
                        let inner = values[0].as_seq();
                        let name = TypeName(inner[0].as_name());
                        let children = match &inner[1] {
                            ParseValue::Dialect(DialectValue::StructChildren(c)) => c.clone(),
                            _ => return Err("expected struct children".into()),
                        };
                        module.structs.push(StructDef {
                            name, visibility: Visibility::Public,
                            generic_params: vec![], derives: vec![],
                            children, span: Self::span_from_values(inner),
                        });
                    }
                    4 => {
                        // *{|@Const <Type> @Literal|}
                        let inner = values[0].as_seq();
                        let name = TypeName(inner[0].as_name());
                        let typ = inner[1].as_type_expr();
                        let value = inner[2].as_literal();
                        module.consts.push(ConstDef {
                            name, visibility: Visibility::Public,
                            typ, value, span: Self::span_from_values(inner),
                        });
                    }
                    5 => {
                        // *(|@Ffi <Ffi>|)
                        let inner = values[0].as_seq();
                        let name = TypeName(inner[0].as_name());
                        match &inner[1] {
                            ParseValue::Dialect(DialectValue::FfiDef(f)) => {
                                let mut f = f.clone();
                                f.library = name;
                                f.span = Self::span_from_values(inner);
                                module.ffi.push(f);
                            }
                            _ => return Err("expected ffi def".into()),
                        }
                    }
                    6 => {
                        // ?[|<Process>|]
                        let inner = values[0].as_seq();
                        let block = inner[0].as_block();
                        module.process = Some(block);
                    }
                    7 => {
                        // *@Newtype <Type>
                        let name = TypeName(values[0].as_name());
                        let wraps = values[1].as_type_expr();
                        module.newtypes.push(NewtypeDef {
                            name, visibility: Visibility::Public,
                            generic_params: vec![], derives: vec![],
                            wraps, span: Span {
                                start: values[0].as_span().start,
                                end: values[1].as_span().end,
                            },
                        });
                    }
                    _ => return Err(format!("unknown root alt {}", alt_idx)),
                }
            }
        }

        Ok(ParseValue::Dialect(DialectValue::Module(module)))
    }

    // ── Module ──────────────────────────────────────────────

    fn build_module(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Module.synth has 3 sequential rules:
        //   *@ObjectExport
        //   *@actionExport
        //   *[:Module *:ObjectImport *:actionImport]
        let mut exports = Vec::new();
        let mut imports = Vec::new();

        // Rule 0: *@ObjectExport
        if let Some(MatchedRule::Sequential(ref items)) = rules.get(0) {
            if let Some(ParseValue::Seq(ref names)) = items.get(0) {
                for n in names {
                    exports.push(ExportItem::Type_(TypeName(n.as_name())));
                }
            }
        }

        // Rule 1: *@actionExport
        if let Some(MatchedRule::Sequential(ref items)) = rules.get(1) {
            if let Some(ParseValue::Seq(ref names)) = items.get(0) {
                for n in names {
                    exports.push(ExportItem::Trait(TraitName(n.as_name())));
                }
            }
        }

        // Rule 2: *[:Module *:ObjectImport *:actionImport]
        if let Some(MatchedRule::Sequential(ref items)) = rules.get(2) {
            if let Some(ParseValue::Seq(ref import_blocks)) = items.get(0) {
                for block in import_blocks {
                    let inner = block.as_seq();
                    let source = TypeName(inner[0].as_name());
                    let mut names = Vec::new();
                    // inner[1] = Seq of object imports
                    if let ParseValue::Seq(ref objs) = inner[1] {
                        for n in objs {
                            names.push(ImportItem::Type_(TypeName(n.as_name())));
                        }
                    }
                    // inner[2] = Seq of action imports
                    if let ParseValue::Seq(ref acts) = inner[2] {
                        for n in acts {
                            names.push(ImportItem::Trait(TraitName(n.as_name())));
                        }
                    }
                    imports.push(ModuleImport { source, names });
                }
            }
        }

        Ok(ParseValue::Dialect(DialectValue::Module(ModuleDef {
            name: TypeName(String::new()), // filled by Root builder
            visibility: Visibility::Public,
            exports,
            imports,
            enums: vec![],
            structs: vec![],
            newtypes: vec![],
            consts: vec![],
            trait_decls: vec![],
            trait_impls: vec![],
            ffi: vec![],
            process: None,
            span: Span { start: 0, end: 0 }, // filled by Root builder
        })))
    }

    // ── Enum ────────────────────────────────────────────────

    fn build_enum(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Enum.synth: one OrderedChoice with 5 alternatives, all *
        let repeated = match rules.into_iter().next() {
            Some(MatchedRule::RepeatedChoice(v)) => v,
            _ => return Ok(ParseValue::Dialect(DialectValue::EnumChildren(vec![]))),
        };

        let mut children = Vec::new();
        for (alt_idx, values) in &repeated {
            let child = match alt_idx {
                0 => {
                    // *@Variant → bare variant
                    EnumChild::Variant {
                        name: VariantName(values[0].as_name()),
                        span: values[0].as_span(),
                    }
                }
                1 => {
                    // *(@Variant <Type>) → data-carrying
                    let inner = values[0].as_seq();
                    EnumChild::DataVariant {
                        name: VariantName(inner[0].as_name()),
                        payload: inner[1].as_type_expr(),
                        span: Self::span_from_values(inner),
                    }
                }
                2 => {
                    // *{@Variant <Struct>} → struct variant
                    let inner = values[0].as_seq();
                    let children = match &inner[1] {
                        ParseValue::Dialect(DialectValue::StructChildren(c)) => {
                            c.iter().filter_map(|sc| match sc {
                                StructChild::TypedField { name, typ, span, .. } =>
                                    Some(StructField { name: name.clone(), visibility: Visibility::Public, typ: typ.clone(), span: span.clone() }),
                                StructChild::SelfTypedField { name, span, .. } =>
                                    Some(StructField { name: FieldName(name.0.clone()), visibility: Visibility::Public, typ: TypeExpr::Named(TypeName(name.0.clone())), span: span.clone() }),
                                _ => None,
                            }).collect()
                        }
                        _ => vec![],
                    };
                    EnumChild::StructVariant {
                        name: VariantName(inner[0].as_name()),
                        fields: children,
                        span: Self::span_from_values(inner),
                    }
                }
                3 => {
                    // *(|@Enum <Enum>|) → nested enum
                    let inner = values[0].as_seq();
                    let name = TypeName(inner[0].as_name());
                    let nested_children = match &inner[1] {
                        ParseValue::Dialect(DialectValue::EnumChildren(c)) => c.clone(),
                        _ => vec![],
                    };
                    EnumChild::NestedEnum(EnumDef {
                        name, visibility: Visibility::Public,
                        generic_params: vec![], derives: vec![],
                        children: nested_children, span: Self::span_from_values(inner),
                    })
                }
                4 => {
                    // *{|@Struct <Struct>|} → nested struct
                    let inner = values[0].as_seq();
                    let name = TypeName(inner[0].as_name());
                    let nested_children = match &inner[1] {
                        ParseValue::Dialect(DialectValue::StructChildren(c)) => c.clone(),
                        _ => vec![],
                    };
                    EnumChild::NestedStruct(StructDef {
                        name, visibility: Visibility::Public,
                        generic_params: vec![], derives: vec![],
                        children: nested_children, span: Self::span_from_values(inner),
                    })
                }
                _ => return Err(format!("unknown enum alt {}", alt_idx)),
            };
            children.push(child);
        }

        Ok(ParseValue::Dialect(DialectValue::EnumChildren(children)))
    }

    // ── Struct ──────────────────────────────────────────────

    fn build_struct(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Struct.synth: one OrderedChoice with 4 alternatives, all *
        let repeated = match rules.into_iter().next() {
            Some(MatchedRule::RepeatedChoice(v)) => v,
            _ => return Ok(ParseValue::Dialect(DialectValue::StructChildren(vec![]))),
        };

        let mut children = Vec::new();
        for (alt_idx, values) in &repeated {
            let child = match alt_idx {
                0 => {
                    // *(@Field <Type>) → typed field
                    let inner = values[0].as_seq();
                    StructChild::TypedField {
                        name: FieldName(inner[0].as_name()),
                        visibility: Visibility::Public,
                        typ: inner[1].as_type_expr(),
                        span: Self::span_from_values(inner),
                    }
                }
                1 => {
                    // *@Field → self-typed field
                    StructChild::SelfTypedField {
                        name: FieldName(values[0].as_name()),
                        visibility: Visibility::Public,
                        span: values[0].as_span(),
                    }
                }
                2 => {
                    // *(|@Enum <Enum>|) → nested enum
                    let inner = values[0].as_seq();
                    let name = TypeName(inner[0].as_name());
                    let nested_children = match &inner[1] {
                        ParseValue::Dialect(DialectValue::EnumChildren(c)) => c.clone(),
                        _ => vec![],
                    };
                    StructChild::NestedEnum(EnumDef {
                        name, visibility: Visibility::Public,
                        generic_params: vec![], derives: vec![],
                        children: nested_children, span: Self::span_from_values(inner),
                    })
                }
                3 => {
                    // *{|@Struct <Struct>|} → nested struct
                    let inner = values[0].as_seq();
                    let name = TypeName(inner[0].as_name());
                    let nested_children = match &inner[1] {
                        ParseValue::Dialect(DialectValue::StructChildren(c)) => c.clone(),
                        _ => vec![],
                    };
                    StructChild::NestedStruct(StructDef {
                        name, visibility: Visibility::Public,
                        generic_params: vec![], derives: vec![],
                        children: nested_children, span: Self::span_from_values(inner),
                    })
                }
                _ => return Err(format!("unknown struct alt {}", alt_idx)),
            };
            children.push(child);
        }

        Ok(ParseValue::Dialect(DialectValue::StructChildren(children)))
    }

    // ── Type ────────────────────────────────────────────────

    fn build_type(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Type.synth: one OrderedChoice with 4 alternatives
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("type expected choice".into()),
        };

        let type_expr = match alt_idx {
            0 => {
                // _@_[<TypeApplication>] → InstanceRef
                // values: [Token(@), Seq([Dialect(TypeApp)])]
                let inner = values[1].as_seq();
                match &inner[0] {
                    ParseValue::Dialect(DialectValue::TypeApp(ta)) => {
                        TypeExpr::InstanceRef(ta.clone())
                    }
                    _ => return Err("expected type application".into()),
                }
            }
            1 => {
                // [<TypeApplication>] → Application
                let inner = values[0].as_seq();
                match &inner[0] {
                    ParseValue::Dialect(DialectValue::TypeApp(ta)) => {
                        TypeExpr::Application(ta.clone())
                    }
                    _ => return Err("expected type application".into()),
                }
            }
            2 => {
                // _$_<GenericParam> → Param or BoundedParam
                // values: [Token($), Dialect(TypeExpr)]
                values[1].as_type_expr()
            }
            3 => {
                // :Type → Simple
                TypeExpr::Named(TypeName(values[0].as_name()))
            }
            _ => return Err(format!("unknown type alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::TypeExpr(type_expr)))
    }

    fn build_type_application(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // TypeApplication.synth: Sequential: :Constructor +<Type>
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("type application expected sequential".into()),
        };

        let constructor = TypeName(items[0].as_name());
        let args: Vec<TypeExpr> = match &items[1] {
            ParseValue::Seq(types) => types.iter().map(|t| t.as_type_expr()).collect(),
            other => vec![other.as_type_expr()],
        };

        Ok(ParseValue::Dialect(DialectValue::TypeApp(TypeApplication {
            constructor, args,
        })))
    }

    fn build_generic_param(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // GenericParam.synth: OrderedChoice with 2 alternatives
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("generic param expected choice".into()),
        };

        let type_expr = match alt_idx {
            0 => {
                // :Bound_&_<GenericParam> → BoundedParam
                // Recursive: collects all bounds
                let mut bounds = vec![TypeName(values[0].as_name())];
                // values[1] = Token(&), values[2] = Dialect(TypeExpr)
                Self::collect_bounds(&values[2], &mut bounds);
                TypeExpr::BoundedParam { bounds }
            }
            1 => {
                // @Role → Param
                TypeExpr::Param(TypeParamName(values[0].as_name()))
            }
            _ => return Err(format!("unknown generic param alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::TypeExpr(type_expr)))
    }

    fn collect_bounds(value: &ParseValue, bounds: &mut Vec<TypeName>) {
        match value {
            ParseValue::Dialect(DialectValue::TypeExpr(TypeExpr::BoundedParam { bounds: inner })) => {
                bounds.extend(inner.iter().cloned());
            }
            ParseValue::Dialect(DialectValue::TypeExpr(TypeExpr::Param(name))) => {
                bounds.push(TypeName(name.0.clone()));
            }
            _ => {}
        }
    }

    // ── Expression builders ─────────────────────────────────

    fn build_expr_binary(&self, rules: Vec<MatchedRule>, op: BinOp) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("expr binary expected choice".into()),
        };

        if alt_idx == 0 {
            // <Lower> op <Self> → BinXxx { left, right }
            let left = Box::new(values[0].as_expr());
            // values[1] = Token(operator)
            let right = Box::new(values[2].as_expr());
            let span = Span {
                start: values[0].as_span().start,
                end: values[2].as_span().end,
            };
            let expr = op.make_expr(left, right, span);
            Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
        } else {
            // Fallthrough — return the sub-result directly
            Ok(values.into_iter().next().unwrap())
        }
    }

    fn build_expr_compare(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("expr compare expected choice".into()),
        };

        // alt 6 = fallthrough
        if alt_idx == 6 {
            return Ok(values.into_iter().next().unwrap());
        }

        let left = Box::new(values[0].as_expr());
        let right = Box::new(values[2].as_expr());
        let span = Span {
            start: values[0].as_span().start,
            end: values[2].as_span().end,
        };

        let expr = match alt_idx {
            0 => Expr::BinEq { left, right, span },
            1 => Expr::BinNotEq { left, right, span },
            2 => Expr::BinLt { left, right, span },
            3 => Expr::BinGt { left, right, span },
            4 => Expr::BinLtEq { left, right, span },
            5 => Expr::BinGtEq { left, right, span },
            _ => return Err(format!("unknown compare alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
    }

    fn build_expr_add(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("expr add expected choice".into()),
        };

        if alt_idx == 2 { return Ok(values.into_iter().next().unwrap()); }

        let left = Box::new(values[0].as_expr());
        let right = Box::new(values[2].as_expr());
        let span = Span {
            start: values[0].as_span().start,
            end: values[2].as_span().end,
        };

        let expr = match alt_idx {
            0 => Expr::BinAdd { left, right, span },
            1 => Expr::BinSub { left, right, span },
            _ => return Err(format!("unknown add alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
    }

    fn build_expr_mul(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("expr mul expected choice".into()),
        };

        if alt_idx == 2 { return Ok(values.into_iter().next().unwrap()); }

        let left = Box::new(values[0].as_expr());
        let right = Box::new(values[2].as_expr());
        let span = Span {
            start: values[0].as_span().start,
            end: values[2].as_span().end,
        };

        let expr = match alt_idx {
            0 => Expr::BinMul { left, right, span },
            1 => Expr::BinMod { left, right, span },
            _ => return Err(format!("unknown mul alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
    }

    fn build_expr_atom(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("expr atom expected choice".into()),
        };

        let span = values.first().map(|v| v.as_span()).unwrap_or(Span { start: 0, end: 0 });

        let expr = match alt_idx {
            0 => {
                // _@_:Instance → InstanceRef
                Expr::InstanceRef {
                    name: TypeName(values[1].as_name()),
                    span: Span { start: values[0].as_span().start, end: values[1].as_span().end },
                }
            }
            1 => {
                // :Variant → BareVariant
                Expr::BareVariant {
                    variant: VariantName(values[0].as_name()),
                    span,
                }
            }
            2 => {
                // :Type/:Variant → PathVariant
                Expr::PathVariant {
                    typ: TypeName(values[0].as_name()),
                    variant: VariantName(values[2].as_name()),
                    span: Span { start: values[0].as_span().start, end: values[2].as_span().end },
                }
            }
            3 => {
                // :Type/:method(+<Expr>) → PathCall
                let method = MethodName(values[2].as_name());
                let args: Vec<Expr> = match &values[3] {
                    ParseValue::Seq(v) => v.iter().map(|a| a.as_expr()).collect(),
                    _ => vec![],
                };
                Expr::PathCall {
                    typ: TypeName(values[0].as_name()),
                    method, args,
                    span: Span { start: values[0].as_span().start, end: values.last().unwrap().as_span().end },
                }
            }
            4 => {
                // :Literal → IntLit / FloatLit / StringLit
                match &values[0] {
                    ParseValue::Literal(LiteralValue::Int(v), s) => Expr::IntLit { value: *v, span: s.clone() },
                    ParseValue::Literal(LiteralValue::Float(v), s) => Expr::FloatLit { value: *v, span: s.clone() },
                    ParseValue::Literal(LiteralValue::Str(v), s) => Expr::StringLit { value: v.clone(), span: s.clone() },
                    ParseValue::Literal(LiteralValue::Bool(v), s) => Expr::BoolLit { value: *v, span: s.clone() },
                    _ => return Err("expected literal".into()),
                }
            }
            5 => {
                // [<Body>] → InlineEval
                let inner = values[0].as_seq();
                Expr::InlineEval(inner[0].as_block())
            }
            6 => {
                // (|<Match>|) → Match
                let inner = values[0].as_seq();
                Expr::Match(inner[0].as_match_expr())
            }
            7 => {
                // [|<Loop>|] → Loop
                let inner = values[0].as_seq();
                Expr::Loop(inner[0].as_loop_expr())
            }
            8 => {
                // {| <IterationSource> [<Body>] |} → Iteration
                let inner = values[0].as_seq();
                let (source, binding) = match &inner[0] {
                    ParseValue::Dialect(DialectValue::IterationSource { source, binding }) =>
                        (source.clone(), binding.clone()),
                    _ => (inner[0].as_expr(), Pattern::Wildcard),
                };
                Expr::Iteration(Iteration {
                    binding,
                    source: Box::new(source),
                    body: inner[1].as_block(),
                })
            }
            9 => {
                // {<StructConstruct>} → StructConstruct
                let inner = values[0].as_seq();
                match &inner[0] {
                    ParseValue::Dialect(DialectValue::StructConstruct { typ, fields }) => {
                        Expr::StructConstruct {
                            typ: typ.clone(), fields: fields.clone(),
                            span,
                        }
                    }
                    _ => return Err("expected struct construct".into()),
                }
            }
            _ => return Err(format!("unknown atom alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Expr(expr)))
    }

    // ── Statement ───────────────────────────────────────────

    fn build_statement(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("statement expected choice".into()),
        };

        let stmt = match alt_idx {
            0 => {
                // ^<Expr> → EarlyReturn
                Statement::EarlyReturn(Box::new(values[1].as_expr()))
            }
            1 => {
                // [|<Expr> <Body>|] → Loop with condition
                let inner = values[0].as_seq();
                Statement::Loop(LoopExpr {
                    condition: Some(Box::new(inner[0].as_expr())),
                    body: Block {
                        statements: match &inner[1] {
                            ParseValue::Dialect(DialectValue::Block(b)) => b.statements.clone(),
                            _ => vec![],
                        },
                        tail: match &inner[1] {
                            ParseValue::Dialect(DialectValue::Block(b)) => b.tail.clone(),
                            _ => None,
                        },
                    },
                })
            }
            2 => {
                // [|<Body>|] → Loop without condition
                let inner = values[0].as_seq();
                Statement::Loop(LoopExpr {
                    condition: None,
                    body: inner[0].as_block(),
                })
            }
            3 => {
                // {| <IterationSource> [<Body>] |} → Iteration
                let inner = values[0].as_seq();
                let (source, binding) = match &inner[0] {
                    ParseValue::Dialect(DialectValue::IterationSource { source, binding }) =>
                        (source.clone(), binding.clone()),
                    _ => (inner[0].as_expr(), Pattern::Wildcard),
                };
                Statement::Iteration(Iteration {
                    binding,
                    source: Box::new(source),
                    body: inner[1].as_block(),
                })
            }
            4 => {
                // (@Type <Type>) → LocalTypeDecl
                let inner = values[0].as_seq();
                Statement::LocalTypeDecl {
                    name: TypeName(inner[0].as_name()),
                    typ: inner[1].as_type_expr(),
                    span: Self::span_from_values(inner),
                }
            }
            5 => {
                // _~@_<Mutation> → Mutation
                Statement::Mutation(values[1].as_mutation())
            }
            6 => {
                // _@_<Instance> → Instance
                Statement::Instance(values[1].as_instance())
            }
            7 => {
                // <Expr> → Expr statement
                Statement::Expr(Box::new(values[0].as_expr()))
            }
            _ => return Err(format!("unknown statement alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Statement(stmt)))
    }

    // ── Body ────────────────────────────────────────────────

    fn build_body(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Body.synth: Sequential *<Statement> <Expr>
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("body expected sequential".into()),
        };

        let statements: Vec<Statement> = match &items[0] {
            ParseValue::Seq(stmts) => stmts.iter().map(|s| s.as_statement()).collect(),
            _ => vec![],
        };

        let tail = if items.len() > 1 {
            Some(Box::new(items[1].as_expr()))
        } else {
            None
        };

        Ok(ParseValue::Dialect(DialectValue::Block(Block { statements, tail })))
    }

    // ── Instance ────────────────────────────────────────────

    fn build_instance(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("instance expected choice".into()),
        };

        let instance = match alt_idx {
            0 => {
                // @Instance (<Type>) <Expr>
                let type_ann = values[1].as_seq();
                Instance {
                    name: TypeName(values[0].as_name()),
                    type_annotation: Some(type_ann[0].as_type_expr()),
                    value: Box::new(values[2].as_expr()),
                    span: Self::span_from_values(&values),
                }
            }
            1 => {
                // @Instance <Expr>
                Instance {
                    name: TypeName(values[0].as_name()),
                    type_annotation: None,
                    value: Box::new(values[1].as_expr()),
                    span: Self::span_from_values(&values),
                }
            }
            _ => return Err(format!("unknown instance alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Instance(instance)))
    }

    // ── Mutation ────────────────────────────────────────────

    fn build_mutation(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Mutation.synth: Sequential :Instance.:method(+<Expr>)
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("mutation expected sequential".into()),
        };

        // items: [Name(instance), Token(dot), Name(method), Seq(args)]
        let name = TypeName(items[0].as_name());
        // items[1] = Token(dot)
        let method = MethodName(items[2].as_name());
        let args: Vec<Expr> = match &items[3] {
            ParseValue::Seq(v) => v.iter().map(|a| a.as_expr()).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::Mutation(Mutation {
            name, method, args,
            span: Self::span_from_values(&items),
        })))
    }

    // ── Param ───────────────────────────────────────────────

    fn build_param(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("param expected choice".into()),
        };

        let param = match alt_idx {
            0 => Param::BorrowSelf,       // _:@_Self
            1 => Param::MutBorrowSelf,    // _~@_Self
            2 => Param::OwnedSelf,        // _@_Self
            3 => {
                // _:@_@Param <Type> → BorrowNamed
                Param::BorrowNamed {
                    name: TypeName(values[1].as_name()),
                    typ: values[2].as_type_expr(),
                }
            }
            4 => {
                // _~@_@Param <Type> → MutBorrowNamed
                Param::MutBorrowNamed {
                    name: TypeName(values[1].as_name()),
                    typ: values[2].as_type_expr(),
                }
            }
            5 => {
                // _@_@Param <Type> → Named
                Param::Named {
                    name: TypeName(values[1].as_name()),
                    typ: values[2].as_type_expr(),
                }
            }
            6 => {
                // _@_@Param → Bare
                Param::Bare {
                    name: TypeName(values[1].as_name()),
                }
            }
            _ => return Err(format!("unknown param alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Param(param)))
    }

    // ── Remaining dialect builders ──────────────────────────

    fn build_signature(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Signature.synth: Sequential +<Param> ?<Type>
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("signature expected sequential".into()),
        };

        let params: Vec<Param> = match &items[0] {
            ParseValue::Seq(v) => v.iter().map(|p| p.as_param()).collect(),
            other => vec![other.as_param()],
        };

        let return_type = if items.len() > 1 && !items[1].is_none() {
            Some(items[1].as_type_expr())
        } else {
            None
        };

        Ok(ParseValue::Dialect(DialectValue::MethodSig(MethodSig {
            name: MethodName(String::new()), // filled by caller
            generic_params: vec![],
            params,
            return_type,
            span: Span { start: 0, end: 0 },
        })))
    }

    fn build_method(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Method.synth: Sequential(+<Param> ?<Type>) + Choice(body forms)
        if rules.len() < 2 {
            return Err("method expected 2 rules".into());
        }

        let seq_items = match &rules[0] {
            MatchedRule::Sequential(v) => v,
            _ => return Err("method rule 0 expected sequential".into()),
        };

        let params: Vec<Param> = match &seq_items[0] {
            ParseValue::Seq(v) => v.iter().map(|p| p.as_param()).collect(),
            other => vec![other.as_param()],
        };

        let return_type = if seq_items.len() > 1 && !seq_items[1].is_none() {
            Some(seq_items[1].as_type_expr())
        } else {
            None
        };

        let (body_alt, body_values) = match &rules[1] {
            MatchedRule::Choice(idx, v) => (*idx, v),
            _ => return Err("method rule 1 expected choice".into()),
        };

        let body = match body_alt {
            0 => MethodBody::Block(body_values[0].as_seq()[0].as_block()),
            1 => MethodBody::Match(body_values[0].as_seq()[0].as_match_expr()),
            2 => MethodBody::Loop(body_values[0].as_seq()[0].as_loop_expr()),
            3 => {
                let inner = body_values[0].as_seq();
                let (source, binding) = match &inner[0] {
                    ParseValue::Dialect(DialectValue::IterationSource { source, binding }) =>
                        (source.clone(), binding.clone()),
                    _ => (inner[0].as_expr(), Pattern::Wildcard),
                };
                MethodBody::Iteration(Iteration {
                    binding,
                    source: Box::new(source),
                    body: inner[1].as_block(),
                })
            }
            4 => {
                let inner = body_values[0].as_seq();
                match &inner[0] {
                    ParseValue::Dialect(DialectValue::StructConstruct { typ, fields }) => {
                        MethodBody::StructConstruct {
                            typ: typ.clone(), fields: fields.clone(),
                            span: Span { start: 0, end: 0 },
                        }
                    }
                    _ => return Err("expected struct construct".into()),
                }
            }
            _ => return Err(format!("unknown method body alt {}", body_alt)),
        };

        Ok(ParseValue::Dialect(DialectValue::MethodDef(MethodDef {
            name: MethodName(String::new()), // filled by caller
            generic_params: vec![],
            params, return_type, body,
            span: Span { start: 0, end: 0 },
        })))
    }

    fn build_trait_decl(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // TraitDecl.synth: Sequential [+(@signature <Signature>)]
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("trait decl expected sequential".into()),
        };

        let sigs_seq = items[0].as_seq(); // outer brackets
        let sigs: Vec<MethodSig> = match &sigs_seq[0] {
            ParseValue::Seq(v) => v.iter().map(|s| {
                let inner = s.as_seq();
                let name = MethodName(inner[0].as_name());
                let mut sig = match &inner[1] {
                    ParseValue::Dialect(DialectValue::MethodSig(ms)) => ms.clone(),
                    _ => MethodSig {
                        name: name.clone(), generic_params: vec![],
                        params: vec![], return_type: None, span: Span { start: 0, end: 0 },
                    },
                };
                sig.name = name;
                sig
            }).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::TraitDecl(TraitDeclDef {
            name: TraitName(String::new()), // filled by Root
            visibility: Visibility::Public,
            generic_params: vec![],
            super_traits: vec![],
            associated_types: vec![],
            signatures: sigs,
            span: Span { start: 0, end: 0 },
        })))
    }

    fn build_trait_impl(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // TraitImpl.synth: Sequential <Type> [<TypeImpl>]
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("trait impl expected sequential".into()),
        };

        let type_expr = items[0].as_type_expr();
        let methods_seq = items[1].as_seq();
        let methods: Vec<MethodDef> = match &methods_seq[0] {
            ParseValue::Dialect(DialectValue::Methods(m)) => m.clone(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::TraitImpl(TraitImplDef {
            trait_name: TraitName(String::new()), // filled by Root
            trait_args: vec![],
            typ: type_expr,
            generic_params: vec![],
            methods,
            associated_types: vec![],
            span: Span { start: 0, end: 0 },
        })))
    }

    fn build_type_impl(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // TypeImpl.synth: Sequential +(@method <Method>)
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("type impl expected sequential".into()),
        };

        let methods: Vec<MethodDef> = match &items[0] {
            ParseValue::Seq(v) => v.iter().map(|m| {
                let inner = m.as_seq();
                let name = MethodName(inner[0].as_name());
                let mut method = match &inner[1] {
                    ParseValue::Dialect(DialectValue::MethodDef(md)) => md.clone(),
                    _ => return MethodDef {
                        name, generic_params: vec![], params: vec![],
                        return_type: None, body: MethodBody::Block(Block { statements: vec![], tail: None }),
                        span: Span { start: 0, end: 0 },
                    },
                };
                method.name = name;
                method
            }).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::Methods(methods)))
    }

    fn build_match(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Match.synth: Sequential ?<Expr> +(<Pattern>) <Expr>
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("match expected sequential".into()),
        };

        let target = if !items[0].is_none() {
            Some(Box::new(items[0].as_expr()))
        } else {
            None
        };

        let arms: Vec<MatchArm> = match &items[1] {
            ParseValue::Seq(v) => v.iter().map(|arm_val| {
                let inner = arm_val.as_seq();
                // inner[0] = Seq([patterns]) from delimited, inner[1] = expr result
                let patterns = inner[0].as_seq();
                let pattern = if patterns.len() == 1 {
                    patterns[0].as_pattern()
                } else {
                    Pattern::Wildcard
                };
                let result = Box::new(inner[1].as_expr());
                MatchArm { pattern, guard: None, result }
            }).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::MatchExpr(MatchExpr { target, arms })))
    }

    fn build_pattern(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("pattern expected choice".into()),
        };

        let pattern = match alt_idx {
            0 => {
                // :Variant _@_@Binding → VariantDataPattern
                Pattern::VariantDataPattern {
                    typ: None,
                    variant: VariantName(values[0].as_name()),
                    inner: vec![Pattern::IdentBind {
                        name: TypeName(values[2].as_name()),
                        mutable: false,
                        span: values[2].as_span(),
                    }],
                    span: Self::span_from_values(&values),
                }
            }
            1 => {
                // :Variant → VariantPattern
                Pattern::VariantPattern {
                    typ: None,
                    variant: VariantName(values[0].as_name()),
                    span: values[0].as_span(),
                }
            }
            2 => {
                // "literal" → StringLitPattern
                let content = match &values[0] {
                    ParseValue::Literal(LiteralValue::Str(s), _) => s.clone(),
                    _ => String::new(),
                };
                Pattern::StringLitPattern(content)
            }
            _ => return Err(format!("unknown pattern alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::Pattern(pattern)))
    }

    fn build_loop(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        let (alt_idx, values) = match rules.into_iter().next() {
            Some(MatchedRule::Choice(idx, v)) => (idx, v),
            Some(MatchedRule::RepeatedChoice(mut v)) if v.len() == 1 => v.remove(0),
            _ => return Err("loop expected choice".into()),
        };

        let loop_expr = match alt_idx {
            0 => {
                // <Expr> +<Statement> → conditional loop
                let stmts: Vec<Statement> = match &values[1] {
                    ParseValue::Seq(v) => v.iter().map(|s| s.as_statement()).collect(),
                    other => vec![other.as_statement()],
                };
                LoopExpr {
                    condition: Some(Box::new(values[0].as_expr())),
                    body: Block { statements: stmts, tail: None },
                }
            }
            1 => {
                // +<Statement> → infinite loop
                let stmts: Vec<Statement> = match &values[0] {
                    ParseValue::Seq(v) => v.iter().map(|s| s.as_statement()).collect(),
                    other => vec![other.as_statement()],
                };
                LoopExpr {
                    condition: None,
                    body: Block { statements: stmts, tail: None },
                }
            }
            _ => return Err(format!("unknown loop alt {}", alt_idx)),
        };

        Ok(ParseValue::Dialect(DialectValue::LoopExpr(loop_expr)))
    }

    fn build_process(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Process.synth: Sequential +<Statement>
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("process expected sequential".into()),
        };

        let stmts: Vec<Statement> = match &items[0] {
            ParseValue::Seq(v) => v.iter().map(|s| s.as_statement()).collect(),
            other => vec![other.as_statement()],
        };

        Ok(ParseValue::Dialect(DialectValue::Block(Block { statements: stmts, tail: None })))
    }

    fn build_iteration_source(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // IterationSource.synth: Sequential <Expr>.@Binding
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("iteration source expected sequential".into()),
        };

        // items: [Dialect(Expr), Token(dot), Name(binding)]
        let source = items[0].as_expr();
        // items[1] = Token(dot)
        let binding_name = TypeName(items[2].as_name());
        let binding = Pattern::IdentBind {
            name: binding_name,
            mutable: false,
            span: items[2].as_span(),
        };

        Ok(ParseValue::Dialect(DialectValue::IterationSource { source, binding }))
    }

    fn build_struct_construct(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // StructConstruct.synth: Sequential :Struct +(:Field <Expr>)
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("struct construct expected sequential".into()),
        };

        let typ = TypeName(items[0].as_name());
        let fields: Vec<FieldInit> = match &items[1] {
            ParseValue::Seq(v) => v.iter().map(|f| {
                let inner = f.as_seq();
                FieldInit {
                    name: FieldName(inner[0].as_name()),
                    value: Box::new(inner[1].as_expr()),
                }
            }).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::StructConstruct { typ, fields }))
    }

    fn build_ffi(&self, rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        // Ffi.synth: Sequential +(@foreignFunction <Signature>)
        let items = match rules.into_iter().next() {
            Some(MatchedRule::Sequential(v)) => v,
            _ => return Err("ffi expected sequential".into()),
        };

        let functions: Vec<FfiFunction> = match &items[0] {
            ParseValue::Seq(v) => v.iter().map(|f| {
                let inner = f.as_seq();
                let name = MethodName(inner[0].as_name());
                let sig = match &inner[1] {
                    ParseValue::Dialect(DialectValue::MethodSig(ms)) => ms.clone(),
                    _ => MethodSig {
                        name: name.clone(), generic_params: vec![],
                        params: vec![], return_type: None, span: Span { start: 0, end: 0 },
                    },
                };
                FfiFunction {
                    name, params: sig.params, return_type: sig.return_type,
                    span: sig.span,
                }
            }).collect(),
            _ => vec![],
        };

        Ok(ParseValue::Dialect(DialectValue::FfiDef(FfiDef {
            library: TypeName(String::new()), // filled by Root
            functions,
            span: Span { start: 0, end: 0 },
        })))
    }

    fn span_from_values(values: &[ParseValue]) -> Span {
        let start = values.first().map(|v| v.as_span().start).unwrap_or(0);
        let end = values.last().map(|v| v.as_span().end).unwrap_or(0);
        Span { start, end }
    }
}

// ── Helper types ──────────────────────────────

/// Binary operation kind for the expression builder.
enum BinOp { Or, And }

impl BinOp {
    fn make_expr(&self, left: Box<Expr>, right: Box<Expr>, span: Span) -> Expr {
        match self {
            BinOp::Or => Expr::BinOr { left, right, span },
            BinOp::And => Expr::BinAnd { left, right, span },
        }
    }
}

