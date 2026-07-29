//! Frontend-neutral embedded program AST.

use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Generics, Ident, Pat, Token, Type, Visibility};

/// An embedded program before semantic resolution.
#[derive(Clone, Debug)]
pub struct Program {
    pub attributes: Vec<Attribute>,
    pub signature: Signature,
    pub relations: Vec<Relation>,
    pub rules: Vec<Rule>,
}

/// Name and visibility of the generated program type.
#[derive(Clone, Debug)]
pub struct Signature {
    pub visibility: Visibility,
    pub name: Ident,
    pub generics: Generics,
}

/// A typed relation declaration. Column types are Rust types.
#[derive(Clone, Debug)]
pub struct Relation {
    pub name: Ident,
    pub columns: Punctuated<Type, Token![,]>,
}

/// A Datalog rule with one or more heads and a conjunctive body.
#[derive(Clone, Debug)]
pub struct Rule {
    pub heads: Punctuated<Atom, Token![,]>,
    pub body: Punctuated<BodyItem, Token![,]>,
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

/// One relational aggregate clause.
#[derive(Clone, Debug)]
pub struct Aggregate {
    pub binding: Ident,
    pub operator: Ident,
    pub arguments: Punctuated<Expr, Token![,]>,
    pub source: Atom,
}

/// A relation application. Arguments stay as opaque Rust expressions.
#[derive(Clone, Debug)]
pub struct Atom {
    pub relation: Ident,
    pub arguments: Punctuated<Expr, Token![,]>,
}
