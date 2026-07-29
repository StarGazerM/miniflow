//! Thin `FlowLog` surface parser.
//!
//! The frontend recognizes `FlowLog` declaration and rule spelling, then lowers
//! directly into the shared embedded AST. Rust remains the expression and type
//! language, so this module does not duplicate `FlowLog`'s arithmetic evaluator,
//! typechecker, I/O directives, or standalone-code generator.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, BinOp, Expr, ExprBinary, ExprCall, ExprPath, Generics, Ident, Result, Token, Type,
    WhereClause, parenthesized, parse_quote,
};

use miniflow_core::syntax::{Aggregate, Atom, BodyItem, Program, Relation, Rule, Signature};

mod kw {
    syn::custom_keyword!(decl);
    syn::custom_keyword!(input);
    syn::custom_keyword!(output);
    syn::custom_keyword!(printsize);
}

/// Parse an embedded FlowLog-syntax program into the shared compiler AST.
///
/// # Errors
///
/// Returns a syntax diagnostic when the token stream is not valid embedded
/// `FlowLog` syntax.
pub fn parse(tokens: TokenStream) -> Result<Program> {
    syn::parse2::<FlowLogProgram>(tokens).map(|program| program.0)
}

/// Parse, compile, and format a canonical Rust expansion.
///
/// # Errors
///
/// Returns any frontend, semantic, or emitted-Rust diagnostic.
pub fn compile_canonical(tokens: TokenStream) -> Result<String> {
    miniflow_core::compile_canonical(parse(tokens)?)
}

struct FlowLogProgram(Program);

impl Parse for FlowLogProgram {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut attributes = Attribute::parse_inner(input)?;
        let signature = parse_signature(input)?;
        let mut relations = Vec::new();
        let mut rules = Vec::new();
        let mut outputs = Vec::new();

        while !input.is_empty() {
            if input.peek(Token![.]) {
                parse_directive(input, &mut relations, &mut outputs)?;
            } else {
                rules.push(parse_rule(input)?);
            }
        }

        if !outputs.is_empty() {
            let output_attribute: Attribute = parse_quote!(#![output(#(#outputs),*)]);
            attributes.push(output_attribute);
        }

        Ok(Self(Program {
            attributes,
            signature,
            relations,
            rules,
        }))
    }
}

fn parse_signature(input: ParseStream<'_>) -> Result<Signature> {
    let visibility = input.parse()?;
    input.parse::<Token![struct]>()?;
    let name = input.parse()?;
    let mut generics: Generics = input.parse()?;
    if input.peek(Token![where]) {
        generics.where_clause = Some(input.parse::<WhereClause>()?);
    }
    input.parse::<Token![;]>()?;
    Ok(Signature {
        visibility,
        name,
        generics,
    })
}

fn parse_directive(
    input: ParseStream<'_>,
    relations: &mut Vec<Relation>,
    outputs: &mut Vec<Ident>,
) -> Result<()> {
    input.parse::<Token![.]>()?;
    if input.peek(kw::decl) {
        input.parse::<kw::decl>()?;
        relations.push(parse_relation(input)?);
    } else if input.peek(kw::input) {
        input.parse::<kw::input>()?;
        let _: Ident = input.parse()?;
        if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let _: TokenStream = content.parse()?;
        }
    } else if input.peek(kw::output) {
        input.parse::<kw::output>()?;
        outputs.push(input.parse()?);
        if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let _: TokenStream = content.parse()?;
        }
    } else if input.peek(kw::printsize) {
        input.parse::<kw::printsize>()?;
        outputs.push(input.parse()?);
    } else {
        return Err(input.error(
            "unsupported MiniFlow directive; expected `.decl`, `.input`, `.output`, or `.printsize`",
        ));
    }
    Ok(())
}

fn parse_relation(input: ParseStream<'_>) -> Result<Relation> {
    let name = input.parse()?;
    let content;
    parenthesized!(content in input);
    let mut columns = Punctuated::new();
    while !content.is_empty() {
        let _: Ident = content.parse()?;
        content.parse::<Token![:]>()?;
        columns.push(flowlog_type(content.parse()?));
        if content.is_empty() {
            break;
        }
        content.parse::<Token![,]>()?;
    }
    Ok(Relation { name, columns })
}

fn flowlog_type(ty: Type) -> Type {
    let Type::Path(path) = &ty else {
        return ty;
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return ty;
    }
    let replacement = match path.path.segments[0].ident.to_string().as_str() {
        "int8" => Some(quote!(i8)),
        "int16" => Some(quote!(i16)),
        "int32" => Some(quote!(i32)),
        "int64" => Some(quote!(i64)),
        "uint8" => Some(quote!(u8)),
        "uint16" => Some(quote!(u16)),
        "uint32" => Some(quote!(u32)),
        "uint64" => Some(quote!(u64)),
        "float32" => Some(quote!(f32)),
        "float64" => Some(quote!(f64)),
        "string" => Some(quote!(String)),
        "bool" => Some(quote!(bool)),
        _ => None,
    };
    replacement
        .and_then(|tokens| syn::parse2(tokens).ok())
        .unwrap_or(ty)
}

fn parse_rule(input: ParseStream<'_>) -> Result<Rule> {
    let mut head = parse_atom(input)?;
    if input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        return Ok(Rule {
            heads: Punctuated::from_iter([head]),
            body: Punctuated::new(),
        });
    }

    input.parse::<Token![:]>()?;
    input.parse::<Token![-]>()?;
    let mut body = Punctuated::new();
    loop {
        let (item, terminator_consumed) = parse_body_item(input)?;
        body.push_value(item);
        if terminator_consumed {
            break;
        }
        if input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            break;
        }
        body.push_punct(input.parse::<Token![,]>()?);
    }

    lower_head_aggregate(&mut head, &mut body)?;
    Ok(Rule {
        heads: Punctuated::from_iter([head]),
        body,
    })
}

fn parse_body_item(input: ParseStream<'_>) -> Result<(BodyItem, bool)> {
    if input.peek(Token![!]) {
        input.parse::<Token![!]>()?;
        return Ok((BodyItem::NegatedAtom(parse_atom(input)?), false));
    }
    if looks_like_atom(input) {
        return Ok((BodyItem::Atom(parse_atom(input)?), false));
    }

    let (comparison, terminator_consumed) = parse_comparison(input)?;
    Ok((BodyItem::Condition(comparison), terminator_consumed))
}

fn looks_like_atom(input: ParseStream<'_>) -> bool {
    input.peek(Ident) && input.peek2(syn::token::Paren)
}

fn parse_atom(input: ParseStream<'_>) -> Result<Atom> {
    let relation = input.parse()?;
    let content;
    parenthesized!(content in input);
    let arguments = content.parse_terminated(Expr::parse, Token![,])?;
    Ok(Atom {
        relation,
        arguments,
    })
}

fn parse_comparison(input: ParseStream<'_>) -> Result<(Expr, bool)> {
    let mut left_tokens = TokenStream::new();
    while !comparison_starts(input) {
        if input.is_empty() || input.peek(Token![,]) {
            return Err(input.error("expected a FlowLog comparison predicate"));
        }
        left_tokens.extend([input.parse::<proc_macro2::TokenTree>()?]);
    }
    let left = syn::parse2(left_tokens)?;
    let op = parse_comparison_operator(input)?;
    let mut right_tokens = TokenStream::new();
    let mut terminator_consumed = false;
    while !input.is_empty() && !input.peek(Token![,]) {
        if input.peek(Token![.]) {
            let fork = input.fork();
            fork.parse::<Token![.]>()?;
            if matches!(
                fork.parse::<proc_macro2::TokenTree>(),
                Ok(proc_macro2::TokenTree::Literal(_))
            ) {
                right_tokens.extend([input.parse::<proc_macro2::TokenTree>()?]);
                let field = input.parse::<proc_macro2::TokenTree>()?;
                if let Some(literal) = literal_with_rule_terminator(&field)? {
                    right_tokens.extend(literal);
                    terminator_consumed = true;
                    break;
                }
                right_tokens.extend([field]);
                continue;
            }
            break;
        }
        let token = input.parse::<proc_macro2::TokenTree>()?;
        if let Some(literal) = literal_with_rule_terminator(&token)? {
            right_tokens.extend(literal);
            terminator_consumed = true;
            break;
        }
        right_tokens.extend([token]);
    }
    if right_tokens.is_empty() {
        return Err(input.error("expected the right side of a FlowLog comparison"));
    }
    let right = syn::parse2(right_tokens)?;
    Ok((
        Expr::Binary(ExprBinary {
            attrs: Vec::new(),
            left: Box::new(left),
            op,
            right: Box::new(right),
        }),
        terminator_consumed,
    ))
}

fn literal_with_rule_terminator(token: &proc_macro2::TokenTree) -> Result<Option<TokenStream>> {
    let proc_macro2::TokenTree::Literal(literal) = token else {
        return Ok(None);
    };
    let text = literal.to_string();
    let Some(number) = text.strip_suffix('.') else {
        return Ok(None);
    };
    if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
        return Ok(None);
    }
    number.parse().map(Some).map_err(|_| {
        syn::Error::new(
            literal.span(),
            "invalid numeric literal before FlowLog rule terminator",
        )
    })
}

fn comparison_starts(input: ParseStream<'_>) -> bool {
    input.peek(Token![=])
        || input.peek(Token![==])
        || input.peek(Token![!=])
        || input.peek(Token![<])
        || input.peek(Token![<=])
        || input.peek(Token![>])
        || input.peek(Token![>=])
}

fn parse_comparison_operator(input: ParseStream<'_>) -> Result<BinOp> {
    if input.peek(Token![==]) {
        Ok(BinOp::Eq(input.parse()?))
    } else if input.peek(Token![!=]) {
        Ok(BinOp::Ne(input.parse()?))
    } else if input.peek(Token![<=]) {
        Ok(BinOp::Le(input.parse()?))
    } else if input.peek(Token![>=]) {
        Ok(BinOp::Ge(input.parse()?))
    } else if input.peek(Token![<]) {
        Ok(BinOp::Lt(input.parse()?))
    } else if input.peek(Token![>]) {
        Ok(BinOp::Gt(input.parse()?))
    } else if input.peek(Token![=]) {
        let equals = input.parse::<Token![=]>()?;
        Ok(BinOp::Eq(Token![==]([equals.span; 2])))
    } else {
        Err(input.error("expected a FlowLog comparison operator"))
    }
}

fn lower_head_aggregate(head: &mut Atom, body: &mut Punctuated<BodyItem, Token![,]>) -> Result<()> {
    let mut aggregate = None;
    for (position, argument) in head.arguments.iter_mut().enumerate() {
        let Expr::Call(call) = argument else {
            continue;
        };
        let Some(operator) = aggregate_operator(call) else {
            continue;
        };
        if aggregate.is_some() {
            return Err(syn::Error::new_spanned(
                call,
                "MiniFlow supports one FlowLog head aggregate per rule",
            ));
        }
        let binding = Ident::new(
            &format!("__miniflow_aggregate_{position}"),
            Span::call_site(),
        );
        let arguments = call.args.clone();
        *argument = parse_quote!(#binding);
        aggregate = Some((binding, operator, arguments));
    }

    let Some((binding, operator, arguments)) = aggregate else {
        return Ok(());
    };
    let source_position = body
        .iter()
        .rposition(|item| matches!(item, BodyItem::Atom(_)))
        .ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "FlowLog head aggregate requires a positive source atom",
            )
        })?;
    let mut items = body.iter().cloned().collect::<Vec<_>>();
    let BodyItem::Atom(source) = items.remove(source_position) else {
        unreachable!("aggregate source position selected a positive atom");
    };
    items.insert(
        source_position,
        BodyItem::Aggregate(Aggregate {
            binding,
            operator,
            arguments,
            source,
        }),
    );
    *body = Punctuated::from_iter(items);
    Ok(())
}

fn aggregate_operator(call: &ExprCall) -> Option<Ident> {
    let Expr::Path(ExprPath {
        qself: None, path, ..
    }) = call.func.as_ref()
    else {
        return None;
    };
    let ident = path.get_ident()?;
    match ident.to_string().as_str() {
        "min" | "max" | "sum" | "mean" | "count" => Some(ident.clone()),
        "average" => Some(Ident::new("mean", ident.span())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::parse;
    use miniflow_core::lower;

    #[test]
    fn parses_flowlog_declarations_rules_directives_and_types() {
        let program = parse(quote! {
            pub struct Reach;

            .decl source(value: int32)
            .input source(IO = "file", filename = "Source.csv")
            .decl arc(source: int32, target: int32)
            .decl reach(value: int32)

            reach(x) :- source(x).
            reach(y) :- reach(x), arc(x, y).
            .output reach
        })
        .unwrap();

        assert_eq!(program.relations.len(), 3);
        assert_eq!(program.rules.len(), 2);
        let hir = lower(program).unwrap();
        assert!(hir.outputs.is_some());
    }

    #[test]
    fn parses_comparison_and_head_aggregate() {
        let program = parse(quote! {
            struct Summary;
            .decl value(value: int32)
            .decl small(value: int32)
            .decl least(value: int32)

            small(x) :- value(x), x < 10 .
            least(min(x)) :- value(x).
        })
        .unwrap();

        let hir = lower(program).unwrap();
        assert_eq!(hir.rules.len(), 2);
    }
}
