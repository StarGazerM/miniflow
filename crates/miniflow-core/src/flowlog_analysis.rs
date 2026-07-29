//! FlowLog-compatible expression and type analysis.
//!
//! These functions produce semantic fingerprint inputs and eligibility
//! information. They do not emit Rust and are shared by planning layers and
//! the compatibility renderer.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Ident;
use syn::Expr;

use crate::flowlog_fp;
use crate::flowlog_fp::TransformationArgument;
use crate::hir::Relation;

pub(crate) fn variable_name(expression: &Expr) -> Option<String> {
    expression_variable_ident(expression).map(|ident| ident.to_string())
}

pub(crate) fn expression_variables(expression: &Expr) -> Vec<String> {
    struct Variables(Vec<String>);
    impl syn::visit::Visit<'_> for Variables {
        fn visit_expr_path(&mut self, path: &syn::ExprPath) {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
            {
                self.0.push(path.path.segments[0].ident.to_string());
            }
        }

        fn visit_expr_call(&mut self, call: &syn::ExprCall) {
            for argument in &call.args {
                syn::visit::Visit::visit_expr(self, argument);
            }
        }
    }
    let mut variables = Variables(Vec::new());
    syn::visit::Visit::visit_expr(&mut variables, expression);
    variables.0
}

pub(crate) fn binary_expression_variables(expression: &syn::ExprBinary) -> BTreeSet<String> {
    expression_variables(&expression.left)
        .into_iter()
        .chain(expression_variables(&expression.right))
        .collect()
}

pub(crate) fn flowlog_variable(argument: TransformationArgument) -> flowlog_fp::ArithmeticArgument {
    flowlog_fp::ArithmeticArgument {
        init: flowlog_fp::FactorArgument::Var(argument),
        rest: Vec::new(),
    }
}

pub(crate) fn flowlog_arithmetic(
    expression: &Expr,
    bindings: &BTreeMap<String, usize>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::ArithmeticArgument> {
    flowlog_arithmetic_with(
        expression,
        &|name| {
            bindings
                .get(name)
                .map(|&index| TransformationArgument::KV((false, index)))
        },
        data_type,
    )
}

pub(crate) fn flowlog_arithmetic_with(
    expression: &Expr,
    resolve: &impl Fn(&str) -> Option<TransformationArgument>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::ArithmeticArgument> {
    if let Expr::Paren(paren) = expression {
        return flowlog_arithmetic_with(&paren.expr, resolve, data_type);
    }
    if let Expr::Binary(binary) = expression {
        let mut left = flowlog_arithmetic_with(&binary.left, resolve, data_type)?;
        left.rest.push((
            flowlog_arithmetic_operator(&binary.op)?,
            flowlog_factor_with(&binary.right, resolve, data_type)?,
        ));
        return Some(left);
    }
    Some(flowlog_fp::ArithmeticArgument {
        init: flowlog_factor_with(expression, resolve, data_type)?,
        rest: Vec::new(),
    })
}

fn flowlog_factor_with(
    expression: &Expr,
    resolve: &impl Fn(&str) -> Option<TransformationArgument>,
    data_type: &syn::Type,
) -> Option<flowlog_fp::FactorArgument> {
    match expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            flowlog_factor_with(&unary.expr, resolve, data_type)
        }
        Expr::Path(_) => Some(flowlog_fp::FactorArgument::Var(resolve(&variable_name(
            expression,
        )?)?)),
        Expr::Lit(_) => Some(flowlog_fp::FactorArgument::Const(flowlog_constant(
            expression, data_type,
        )?)),
        Expr::Paren(paren) => Some(flowlog_fp::FactorArgument::Group(Box::new(
            flowlog_arithmetic_with(&paren.expr, resolve, data_type)?,
        ))),
        Expr::Tuple(tuple) => {
            let syn::Type::Tuple(types) = data_type else {
                return None;
            };
            Some(flowlog_fp::FactorArgument::Tuple {
                fields: tuple
                    .elems
                    .iter()
                    .zip(&types.elems)
                    .map(|(field, data_type)| flowlog_arithmetic_with(field, resolve, data_type))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        Expr::Field(field) => {
            let base_type = match field.base.as_ref() {
                Expr::Tuple(tuple) => {
                    let fields = std::iter::repeat_n(data_type.clone(), tuple.elems.len());
                    syn::parse_quote! { (#(#fields,)*) }
                }
                _ => data_type.clone(),
            };
            Some(flowlog_fp::FactorArgument::TupleProj {
                tuple: Box::new(flowlog_arithmetic_with(&field.base, resolve, &base_type)?),
                index: match &field.member {
                    syn::Member::Unnamed(index) => index.index as usize,
                    syn::Member::Named(_) => return None,
                },
            })
        }
        Expr::Call(call) => {
            let Expr::Path(function) = call.func.as_ref() else {
                return None;
            };
            if function.qself.is_some()
                || function.path.leading_colon.is_some()
                || function.path.segments.len() > 1
                    && function.path.segments.first()?.ident != "udf"
            {
                return None;
            }
            if function.path.segments.last()?.ident == "OrderedFloat" {
                let mut arguments = call.args.iter();
                let argument = arguments.next()?;
                if arguments.next().is_some() {
                    return None;
                }
                return Some(flowlog_fp::FactorArgument::Const(flowlog_constant(
                    argument, data_type,
                )?));
            }
            let name = function.path.segments.last()?.ident.to_string();
            let builtin = match name.as_str() {
                "strlen" => Some(flowlog_fp::BuiltinOperator::Strlen),
                "cat" => Some(flowlog_fp::BuiltinOperator::Cat),
                _ => None,
            };
            if let Some(op) = builtin {
                return Some(flowlog_fp::FactorArgument::Builtin {
                    op,
                    args: call
                        .args
                        .iter()
                        .map(|argument| flowlog_arithmetic_with(argument, resolve, data_type))
                        .collect::<Option<Vec<_>>>()?,
                });
            }
            Some(flowlog_fp::FactorArgument::FnCall {
                name,
                args: call
                    .args
                    .iter()
                    .map(|argument| flowlog_arithmetic_with(argument, resolve, data_type))
                    .collect::<Option<Vec<_>>>()?,
            })
        }
        _ => None,
    }
}

fn flowlog_arithmetic_operator(operator: &syn::BinOp) -> Option<flowlog_fp::ArithmeticOperator> {
    match operator {
        syn::BinOp::Add(_) => Some(flowlog_fp::ArithmeticOperator::Plus),
        syn::BinOp::Sub(_) => Some(flowlog_fp::ArithmeticOperator::Minus),
        syn::BinOp::Mul(_) => Some(flowlog_fp::ArithmeticOperator::Multiply),
        syn::BinOp::Div(_) => Some(flowlog_fp::ArithmeticOperator::Divide),
        syn::BinOp::Rem(_) => Some(flowlog_fp::ArithmeticOperator::Modulo),
        _ => None,
    }
}

pub(crate) fn flowlog_comparison_operator(
    operator: &syn::BinOp,
) -> Option<flowlog_fp::ComparisonOperator> {
    match operator {
        syn::BinOp::Eq(_) => Some(flowlog_fp::ComparisonOperator::Equal),
        syn::BinOp::Ne(_) => Some(flowlog_fp::ComparisonOperator::NotEqual),
        syn::BinOp::Gt(_) => Some(flowlog_fp::ComparisonOperator::GreaterThan),
        syn::BinOp::Ge(_) => Some(flowlog_fp::ComparisonOperator::GreaterEqualThan),
        syn::BinOp::Lt(_) => Some(flowlog_fp::ComparisonOperator::LessThan),
        syn::BinOp::Le(_) => Some(flowlog_fp::ComparisonOperator::LessEqualThan),
        _ => None,
    }
}

pub(crate) fn flowlog_constant(
    expression: &Expr,
    data_type: &syn::Type,
) -> Option<flowlog_fp::Constant> {
    let Expr::Lit(literal) = expression else {
        return None;
    };
    let text = match &literal.lit {
        syn::Lit::Int(value) => value.base10_digits().to_owned(),
        syn::Lit::Float(value) => value.base10_digits().to_owned(),
        syn::Lit::Str(value) => value.value(),
        syn::Lit::Bool(value) => {
            if value.value {
                "True".to_owned()
            } else {
                "False".to_owned()
            }
        }
        _ => return None,
    };
    Some(flowlog_fp::Constant {
        text,
        ty: flowlog_data_type(data_type)?,
    })
}

pub(crate) fn flowlog_data_type(data_type: &syn::Type) -> Option<flowlog_fp::DataType> {
    match data_type {
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            match segment.ident.to_string().as_str() {
                "i8" => Some(flowlog_fp::DataType::Int8),
                "i16" => Some(flowlog_fp::DataType::Int16),
                "i32" => Some(flowlog_fp::DataType::Int32),
                "i64" => Some(flowlog_fp::DataType::Int64),
                "u8" => Some(flowlog_fp::DataType::UInt8),
                "u16" => Some(flowlog_fp::DataType::UInt16),
                "u32" => Some(flowlog_fp::DataType::UInt32),
                "u64" => Some(flowlog_fp::DataType::UInt64),
                "f32" => Some(flowlog_fp::DataType::Float32),
                "f64" => Some(flowlog_fp::DataType::Float64),
                "String" => Some(flowlog_fp::DataType::String),
                "bool" => Some(flowlog_fp::DataType::Bool),
                "OrderedFloat" => {
                    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                        return None;
                    };
                    let syn::GenericArgument::Type(inner) = arguments.args.first()? else {
                        return None;
                    };
                    flowlog_data_type(inner)
                }
                _ => None,
            }
        }
        syn::Type::Tuple(tuple) => Some(flowlog_fp::DataType::FixedTuple(
            tuple
                .elems
                .iter()
                .map(flowlog_data_type)
                .collect::<Option<Vec<_>>>()?,
        )),
        _ => None,
    }
}

pub(crate) fn expression_type(
    expression: &Expr,
    bindings: &BTreeMap<String, usize>,
    relation: &Relation,
) -> Option<syn::Type> {
    if let Some(name) = dereferenced_variable_name(expression)
        && let Some(index) = bindings.get(&name)
    {
        return relation.columns.get(*index).cloned();
    }
    match expression {
        Expr::Binary(binary) => expression_type(&binary.left, bindings, relation)
            .or_else(|| expression_type(&binary.right, bindings, relation)),
        Expr::Paren(paren) => expression_type(&paren.expr, bindings, relation),
        Expr::Unary(unary) => expression_type(&unary.expr, bindings, relation),
        Expr::Tuple(tuple) => {
            let fields = tuple
                .elems
                .iter()
                .map(|field| expression_type(field, bindings, relation))
                .collect::<Option<Vec<_>>>()?;
            Some(syn::parse_quote! { (#(#fields,)*) })
        }
        Expr::Field(field) => {
            let syn::Type::Tuple(tuple) = expression_type(&field.base, bindings, relation)? else {
                return None;
            };
            let syn::Member::Unnamed(index) = &field.member else {
                return None;
            };
            tuple.elems.get(index.index as usize).cloned()
        }
        Expr::Call(call) => call
            .args
            .iter()
            .find_map(|argument| expression_type(argument, bindings, relation)),
        _ => None,
    }
}

pub(crate) fn flowlog_copy_type(data_type: &syn::Type) -> bool {
    matches!(
        data_type,
        syn::Type::Path(path)
            if matches!(
                path.path.segments.last().map(|segment| segment.ident.to_string()).as_deref(),
                Some(
                    "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                        | "f32" | "f64" | "bool" | "OrderedFloat"
                )
            )
    )
}

pub(crate) fn dereferenced_variable_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => variable_name(&unary.expr),
        _ => variable_name(expression),
    }
}

pub(crate) fn expression_variable_ident(expression: &Expr) -> Option<Ident> {
    match expression {
        Expr::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1 =>
        {
            Some(path.path.segments[0].ident.clone())
        }
        _ => None,
    }
}

pub(crate) fn expression_mentions_ident(expression: &Expr, searched: &Ident) -> bool {
    struct Finder<'a> {
        searched: &'a Ident,
        found: bool,
    }

    impl syn::visit::Visit<'_> for Finder<'_> {
        fn visit_expr_path(&mut self, path: &syn::ExprPath) {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].ident == *self.searched
            {
                self.found = true;
            }
            syn::visit::visit_expr_path(self, path);
        }
    }

    let mut finder = Finder {
        searched,
        found: false,
    };
    syn::visit::Visit::visit_expr(&mut finder, expression);
    finder.found
}
