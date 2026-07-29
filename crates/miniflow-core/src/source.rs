//! Syntax-neutral source forms consumed by semantic lowering.
//!
//! A source reader owns its grammar and constructs these forms directly.
//! Rust syntax trees are retained only where Rust is the host language for
//! attributes, types, expressions, patterns, visibility, and generics.

use syn::{Attribute, Expr, Generics, Ident, Pat, Type, Visibility};

/// A source program before semantic resolution.
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

/// A typed relation declaration. Column types are host Rust types.
#[derive(Clone, Debug)]
pub struct Relation {
    pub name: Ident,
    pub columns: Vec<Type>,
}

/// A Datalog rule with one or more heads and a conjunctive body.
#[derive(Clone, Debug)]
pub struct Rule {
    pub heads: Vec<Atom>,
    pub body: Vec<BodyItem>,
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
    pub arguments: Vec<Expr>,
    pub source: Atom,
}

/// A relation application. Arguments remain opaque host Rust expressions.
#[derive(Clone, Debug)]
pub struct Atom {
    pub relation: Ident,
    pub arguments: Vec<Expr>,
}
