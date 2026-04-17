/// Builder — per-dialect sema-core type constructors.
///
/// Each method takes ParseValues from the engine and
/// constructs sema-core types. All sema-core knowledge
/// lives here — the engine is generic.

use aski_core::*;
use sema_core::*;
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
            ArchivedDialectKind::ExprOr => self.build_expr_binop(rules, 0),
            ArchivedDialectKind::ExprAnd => self.build_expr_binop(rules, 1),
            ArchivedDialectKind::ExprCompare => self.build_expr_compare(rules),
            ArchivedDialectKind::ExprAdd => self.build_expr_binop(rules, 2),
            ArchivedDialectKind::ExprMul => self.build_expr_binop(rules, 3),
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
            ArchivedDialectKind::StructConstruct => self.build_struct_construct(rules),
            ArchivedDialectKind::Ffi => self.build_ffi(rules),
            _ => Err(format!("no builder for dialect")),
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
                let field = FieldName(postfix[0].as_name()); // dot is Token, field is next
                let end = postfix.last().unwrap().as_span().end;
                Expr::FieldAccess {
                    object: Box::new(base_expr), field,
                    span: Span { start, end },
                }
            }
            1 => {
                // .:method(+<Expr>) → MethodCall
                let method = MethodName(postfix[0].as_name());
                let args_seq = postfix[1].as_seq();
                let args: Vec<Expr> = args_seq.iter().map(|v| v.as_expr()).collect();
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

    // ── Stub builders (to be implemented) ───────────────────

    fn build_root(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_root not yet implemented".into())
    }

    fn build_module(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_module not yet implemented".into())
    }

    fn build_enum(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_enum not yet implemented".into())
    }

    fn build_struct(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_struct not yet implemented".into())
    }

    fn build_body(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_body not yet implemented".into())
    }

    fn build_expr_binop(&self, _rules: Vec<MatchedRule>, _level: usize) -> Result<ParseValue, String> {
        Err("build_expr_binop not yet implemented".into())
    }

    fn build_expr_compare(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_expr_compare not yet implemented".into())
    }

    fn build_expr_atom(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_expr_atom not yet implemented".into())
    }

    fn build_type(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_type not yet implemented".into())
    }

    fn build_type_application(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_type_application not yet implemented".into())
    }

    fn build_generic_param(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_generic_param not yet implemented".into())
    }

    fn build_statement(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_statement not yet implemented".into())
    }

    fn build_instance(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_instance not yet implemented".into())
    }

    fn build_mutation(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_mutation not yet implemented".into())
    }

    fn build_param(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_param not yet implemented".into())
    }

    fn build_signature(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_signature not yet implemented".into())
    }

    fn build_method(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_method not yet implemented".into())
    }

    fn build_trait_decl(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_trait_decl not yet implemented".into())
    }

    fn build_trait_impl(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_trait_impl not yet implemented".into())
    }

    fn build_type_impl(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_type_impl not yet implemented".into())
    }

    fn build_match(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_match not yet implemented".into())
    }

    fn build_pattern(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_pattern not yet implemented".into())
    }

    fn build_loop(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_loop not yet implemented".into())
    }

    fn build_process(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_process not yet implemented".into())
    }

    fn build_struct_construct(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_struct_construct not yet implemented".into())
    }

    fn build_ffi(&self, _rules: Vec<MatchedRule>) -> Result<ParseValue, String> {
        Err("build_ffi not yet implemented".into())
    }
}
