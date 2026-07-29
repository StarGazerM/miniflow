use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Expr, Generics, Ident, Pat, Result, Token, Type, Visibility, WhereClause,
    parenthesized,
};

mod kw {
    syn::custom_keyword!(agg);
    syn::custom_keyword!(relation);
}

/// An embedded `MiniFlow` program before semantic resolution.
#[derive(Clone, Debug)]
pub struct Program {
    pub attributes: Vec<Attribute>,
    pub signature: Signature,
    pub relations: Vec<Relation>,
    pub rules: Vec<Rule>,
}

impl Parse for Program {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let attributes = Attribute::parse_inner(input)?;
        let signature = input.parse()?;
        let mut relations = Vec::new();
        let mut rules = Vec::new();

        while !input.is_empty() {
            if input.peek(kw::relation) {
                relations.push(input.parse()?);
            } else {
                rules.push(input.parse()?);
            }
        }

        Ok(Self {
            attributes,
            signature,
            relations,
            rules,
        })
    }
}

/// Name and visibility of the generated program type.
#[derive(Clone, Debug)]
pub struct Signature {
    pub visibility: Visibility,
    pub name: Ident,
    pub generics: Generics,
}

impl Parse for Signature {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name = input.parse()?;
        let mut generics: Generics = input.parse()?;
        if input.peek(Token![where]) {
            generics.where_clause = Some(input.parse::<WhereClause>()?);
        }
        input.parse::<Token![;]>()?;
        Ok(Self {
            visibility,
            name,
            generics,
        })
    }
}

/// A typed relation declaration. Column types are Rust types.
#[derive(Clone, Debug)]
pub struct Relation {
    pub name: Ident,
    pub columns: Punctuated<Type, Token![,]>,
}

impl Parse for Relation {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<kw::relation>()?;
        let name = input.parse()?;
        let content;
        parenthesized!(content in input);
        let columns = content.parse_terminated(Type::parse, Token![,])?;
        input.parse::<Token![;]>()?;
        Ok(Self { name, columns })
    }
}

/// A Datalog rule with one or more heads and a conjunctive body.
#[derive(Clone, Debug)]
pub struct Rule {
    pub heads: Punctuated<Atom, Token![,]>,
    pub body: Punctuated<BodyItem, Token![,]>,
}

impl Parse for Rule {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let heads = Punctuated::<Atom, Token![,]>::parse_separated_nonempty(input)?;
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
            return Ok(Self {
                heads,
                body: Punctuated::new(),
            });
        }
        parse_long_left_arrow(input)?;
        let body = Punctuated::<BodyItem, Token![,]>::parse_separated_nonempty(input)?;
        input.parse::<Token![;]>()?;
        Ok(Self { heads, body })
    }
}

/// One conjunct in a rule body.
#[derive(Clone, Debug)]
pub enum BodyItem {
    Atom(Atom),
    NegatedAtom(Atom),
    Condition(Expr),
    IfLet { pattern: Pat, expression: Expr },
    Let { pattern: Pat, expression: Expr },
    Generator { pattern: Pat, expression: Expr },
    Aggregate(Aggregate),
}

impl Parse for BodyItem {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(kw::agg) {
            Ok(Self::Aggregate(input.parse()?))
        } else if input.peek(Token![for]) {
            input.parse::<Token![for]>()?;
            let pattern = input.call(Pat::parse_multi)?;
            input.parse::<Token![in]>()?;
            Ok(Self::Generator {
                pattern,
                expression: input.parse()?,
            })
        } else if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            if input.peek(Token![let]) {
                input.parse::<Token![let]>()?;
                let pattern = input.call(Pat::parse_multi)?;
                input.parse::<Token![=]>()?;
                Ok(Self::IfLet {
                    pattern,
                    expression: input.parse()?,
                })
            } else {
                Ok(Self::Condition(input.parse()?))
            }
        } else if input.peek(Token![let]) {
            input.parse::<Token![let]>()?;
            let pattern = input.call(Pat::parse_multi)?;
            input.parse::<Token![=]>()?;
            Ok(Self::Let {
                pattern,
                expression: input.parse()?,
            })
        } else if input.peek(Token![!]) {
            input.parse::<Token![!]>()?;
            Ok(Self::NegatedAtom(input.parse()?))
        } else {
            Ok(Self::Atom(input.parse()?))
        }
    }
}

/// One relational aggregate clause.
#[derive(Clone, Debug)]
pub struct Aggregate {
    pub binding: Ident,
    pub operator: Ident,
    pub arguments: Punctuated<Expr, Token![,]>,
    pub source: Atom,
}

impl Parse for Aggregate {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<kw::agg>()?;
        let binding = input.parse()?;
        input.parse::<Token![=]>()?;
        let operator = input.parse()?;
        let content;
        parenthesized!(content in input);
        let arguments = content.parse_terminated(Expr::parse, Token![,])?;
        input.parse::<Token![in]>()?;
        let source = input.parse()?;
        Ok(Self {
            binding,
            operator,
            arguments,
            source,
        })
    }
}

/// A relation application. Arguments stay as opaque Rust expressions.
#[derive(Clone, Debug)]
pub struct Atom {
    pub relation: Ident,
    pub arguments: Punctuated<Expr, Token![,]>,
}

impl Parse for Atom {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let relation = input.parse()?;
        let content;
        parenthesized!(content in input);
        let arguments = content.parse_terminated(Expr::parse, Token![,])?;
        Ok(Self {
            relation,
            arguments,
        })
    }
}

fn parse_long_left_arrow(input: ParseStream<'_>) -> Result<()> {
    let start = input.span();
    input.parse::<Token![<]>()?;
    input.parse::<Token![-]>()?;
    input.parse::<Token![-]>()?;
    if input.peek(Token![-]) {
        return Err(syn::Error::new(
            Span::call_site(),
            "expected MiniFlow rule arrow `<--`",
        ));
    }
    let _ = start;
    Ok(())
}
