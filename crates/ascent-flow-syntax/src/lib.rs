//! Ascent-shaped syntax frontend for the shared `MiniFlow` compiler core.

use proc_macro2::{Span, TokenStream};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Generics, Pat, Result, Token, Type, WhereClause, parenthesized};

use miniflow_core::syntax::{Aggregate, Atom, BodyItem, Program, Relation, Rule, Signature};

mod kw {
    syn::custom_keyword!(agg);
    syn::custom_keyword!(relation);
}

/// Parse an Ascent-shaped embedded program into the shared compiler AST.
///
/// # Errors
///
/// Returns a syntax diagnostic when the token stream is not valid
/// Ascent-shaped syntax.
pub fn parse(tokens: TokenStream) -> Result<Program> {
    syn::parse2::<ParsedProgram>(tokens).map(|program| program.0)
}

struct ParsedProgram(Program);

impl Parse for ParsedProgram {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attributes = Attribute::parse_inner(input)?;
        let signature = parse_signature(input)?;
        let mut relations = Vec::new();
        let mut rules = Vec::new();

        while !input.is_empty() {
            if input.peek(kw::relation) {
                relations.push(parse_relation(input)?);
            } else {
                rules.push(parse_rule(input)?);
            }
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

fn parse_relation(input: ParseStream<'_>) -> Result<Relation> {
    input.parse::<kw::relation>()?;
    let name = input.parse()?;
    let content;
    parenthesized!(content in input);
    let columns = content.parse_terminated(Type::parse, Token![,])?;
    input.parse::<Token![;]>()?;
    Ok(Relation { name, columns })
}

fn parse_rule(input: ParseStream<'_>) -> Result<Rule> {
    let mut heads = Punctuated::new();
    loop {
        heads.push_value(parse_atom(input)?);
        if !input.peek(Token![,]) {
            break;
        }
        heads.push_punct(input.parse()?);
    }
    if input.peek(Token![;]) {
        input.parse::<Token![;]>()?;
        return Ok(Rule {
            heads,
            body: Punctuated::new(),
        });
    }

    parse_long_left_arrow(input)?;
    let mut body = Punctuated::new();
    loop {
        body.push_value(parse_body_item(input)?);
        if !input.peek(Token![,]) {
            break;
        }
        body.push_punct(input.parse()?);
    }
    input.parse::<Token![;]>()?;
    Ok(Rule { heads, body })
}

fn parse_body_item(input: ParseStream<'_>) -> Result<BodyItem> {
    if input.peek(kw::agg) {
        Ok(BodyItem::Aggregate(parse_aggregate(input)?))
    } else if input.peek(Token![for]) {
        input.parse::<Token![for]>()?;
        let pattern = input.call(Pat::parse_multi)?;
        input.parse::<Token![in]>()?;
        Ok(BodyItem::Generator {
            pattern,
            expression: input.parse()?,
        })
    } else if input.peek(Token![if]) {
        input.parse::<Token![if]>()?;
        if input.peek(Token![let]) {
            input.parse::<Token![let]>()?;
            let pattern = input.call(Pat::parse_multi)?;
            input.parse::<Token![=]>()?;
            Ok(BodyItem::IfLet {
                pattern,
                expression: input.parse()?,
            })
        } else {
            Ok(BodyItem::Condition(input.parse()?))
        }
    } else if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let pattern = input.call(Pat::parse_multi)?;
        input.parse::<Token![=]>()?;
        Ok(BodyItem::Let {
            pattern,
            expression: input.parse()?,
        })
    } else if input.peek(Token![!]) {
        input.parse::<Token![!]>()?;
        Ok(BodyItem::NegatedAtom(parse_atom(input)?))
    } else {
        Ok(BodyItem::Atom(parse_atom(input)?))
    }
}

fn parse_aggregate(input: ParseStream<'_>) -> Result<Aggregate> {
    input.parse::<kw::agg>()?;
    let binding = input.parse()?;
    input.parse::<Token![=]>()?;
    let operator = input.parse()?;
    let content;
    parenthesized!(content in input);
    let arguments = content.parse_terminated(Expr::parse, Token![,])?;
    input.parse::<Token![in]>()?;
    let source = parse_atom(input)?;
    Ok(Aggregate {
        binding,
        operator,
        arguments,
        source,
    })
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

fn parse_long_left_arrow(input: ParseStream<'_>) -> Result<()> {
    input.parse::<Token![<]>()?;
    input.parse::<Token![-]>()?;
    input.parse::<Token![-]>()?;
    if input.peek(Token![-]) {
        return Err(syn::Error::new(
            Span::call_site(),
            "expected AscentFlow rule arrow `<--`",
        ));
    }
    Ok(())
}
