//! Default `MiniFlow` surface parser.

use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Generics, Pat, Result, Token, Type, WhereClause, parenthesized};

use crate::source::{Aggregate, Atom, BodyItem, Program, Relation, Rule, Signature};

mod kw {
    syn::custom_keyword!(agg);
    syn::custom_keyword!(relation);
}

pub(crate) fn parse(tokens: TokenStream) -> Result<Program> {
    (|input: ParseStream<'_>| parse_program(input)).parse2(tokens)
}

fn parse_program(input: ParseStream<'_>) -> Result<Program> {
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

    Ok(Program {
        attributes,
        signature,
        relations,
        rules,
    })
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
    let columns = content
        .parse_terminated(Type::parse, Token![,])?
        .into_iter()
        .collect();
    input.parse::<Token![;]>()?;
    Ok(Relation { name, columns })
}

fn parse_rule(input: ParseStream<'_>) -> Result<Rule> {
    let heads = parse_nonempty(input, parse_atom)?;
    if input.peek(Token![;]) {
        input.parse::<Token![;]>()?;
        return Ok(Rule {
            heads,
            body: Vec::new(),
        });
    }
    input.parse::<Token![:]>()?;
    input.parse::<Token![-]>()?;
    let body = parse_nonempty(input, parse_body_item)?;
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
    let arguments = content
        .parse_terminated(Expr::parse, Token![,])?
        .into_iter()
        .collect();
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
    let arguments = content
        .parse_terminated(Expr::parse, Token![,])?
        .into_iter()
        .collect();
    Ok(Atom {
        relation,
        arguments,
    })
}

fn parse_nonempty<T>(
    input: ParseStream<'_>,
    parser: fn(ParseStream<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    Punctuated::<T, Token![,]>::parse_separated_nonempty_with(input, parser)
        .map(|items| items.into_iter().collect())
}
