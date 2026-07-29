use miniflow_core::compiler::{Layer, Registry};
use miniflow_core::rule_plan::RulePlan;
use miniflow_core::source;
use miniflow_core::{Compiler, PlanRule, ReadSource};
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Generics, Result, Token, Visibility, parse_quote};

struct ManualRulePlanner;

impl Layer for ManualRulePlanner {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<PlanRule, _>(|_context, request, _next| {
            RulePlan::build(request.rule(), request.head())
        });
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct IncrementalDependency {
    target: usize,
}

struct IncrementalAnalysis;

impl Layer for IncrementalAnalysis {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<PlanRule, _>(|context, request, next| {
            context.facts_mut().insert(IncrementalDependency {
                target: request.head().relation.index(),
            });
            next.call(context, request)
        });
        Ok(())
    }
}

mod graph_syntax {
    syn::custom_keyword!(graph);
}

struct GraphDeclaration {
    name: Ident,
}

impl Parse for GraphDeclaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<graph_syntax::graph>()?;
        let name = input.parse()?;
        input.parse::<Token![;]>()?;
        Ok(Self { name })
    }
}

struct GraphSyntax;

impl Layer for GraphSyntax {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<ReadSource, _>(|_, tokens, _next| {
            let declaration = syn::parse2::<GraphDeclaration>(tokens)?;
            Ok(graph_source(declaration.name))
        });
        Ok(())
    }
}

fn binary_relation(name: &str) -> source::Relation {
    source::Relation {
        name: Ident::new(name, Span::call_site()),
        columns: vec![parse_quote!(i32), parse_quote!(i32)],
    }
}

fn graph_source(name: Ident) -> source::Program {
    let head = source::Atom {
        relation: Ident::new("path", Span::call_site()),
        arguments: vec![parse_quote!(x), parse_quote!(y)],
    };
    let body = source::Atom {
        relation: Ident::new("edge", Span::call_site()),
        arguments: vec![parse_quote!(x), parse_quote!(y)],
    };
    source::Program {
        attributes: Vec::new(),
        signature: source::Signature {
            visibility: Visibility::Inherited,
            name,
            generics: Generics::default(),
        },
        relations: vec![binary_relation("edge"), binary_relation("path")],
        rules: vec![source::Rule {
            heads: vec![head],
            body: vec![source::BodyItem::Atom(body)],
        }],
    }
}

fn graph_compiler() -> Compiler {
    let mut compiler = Compiler::base().unwrap();
    compiler.install(&GraphSyntax).unwrap();
    compiler
}

#[test]
fn an_external_syntax_pack_replaces_only_source_reading() {
    let alternate = quote!(graph Program;);
    assert!(
        Compiler::base()
            .unwrap()
            .compile(alternate.clone())
            .is_err()
    );

    let expected = graph_compiler()
        .compile(alternate.clone())
        .unwrap()
        .to_string();
    let mut compiler = graph_compiler();
    compiler.install(&IncrementalAnalysis).unwrap();

    let actual = compiler.compile(alternate).unwrap().to_string();
    let dependencies = compiler
        .context()
        .facts()
        .relation::<IncrementalDependency>();

    assert_eq!(actual, expected);
    assert_eq!(dependencies, [IncrementalDependency { target: 1 }]);
}

#[test]
fn an_external_manual_layer_can_replace_rule_planning() {
    let mut compiler = graph_compiler();
    compiler.install(&ManualRulePlanner).unwrap();

    let expansion = compiler
        .compile(quote!(graph Program;))
        .unwrap()
        .to_string();

    assert!(expansion.contains("__miniflow_rule_0"));
}

#[test]
fn an_external_incremental_layer_can_attach_facts_without_changing_rendering() {
    let baseline = graph_compiler()
        .compile(quote!(graph Program;))
        .unwrap()
        .to_string();
    let mut compiler = graph_compiler();
    compiler.install(&IncrementalAnalysis).unwrap();

    let expansion = compiler
        .compile(quote!(graph Program;))
        .unwrap()
        .to_string();
    let dependencies = compiler
        .context()
        .facts()
        .relation::<IncrementalDependency>();

    assert_eq!(expansion, baseline);
    assert_eq!(dependencies, [IncrementalDependency { target: 1 }]);
}
