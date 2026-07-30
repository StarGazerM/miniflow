use miniflow_core::compiler::{Layer, Registry};
use miniflow_core::rule_plan::RulePlan;
use miniflow_core::source;
use miniflow_core::{CompilerPipeline, PlanRule};
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Generics, Result, Token, Visibility, parse_quote};

struct ManualRulePlanner;

impl Layer for ManualRulePlanner {
    fn install(&self, registry: &mut Registry) -> Result<()> {
        registry.around::<PlanRule, _>(|context, request, next| {
            if request.rule().body.len() != 2 {
                return next.call(context, request);
            }
            context.facts_mut().insert(ReorderedRule);
            RulePlan::build_with_order(request.rule(), request.head(), &[1, 0])
        });
        Ok(())
    }
}

struct ReorderedRule;

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

fn binary_relation(name: &str) -> source::Relation {
    source::Relation {
        name: Ident::new(name, Span::call_site()),
        columns: vec![parse_quote!(i32), parse_quote!(i32)],
    }
}

fn graph_source(name: Ident) -> source::Program {
    let path = source::Atom {
        relation: Ident::new("path", Span::call_site()),
        arguments: vec![parse_quote!(x), parse_quote!(y)],
    };
    let edge = source::Atom {
        relation: Ident::new("edge", Span::call_site()),
        arguments: vec![parse_quote!(x), parse_quote!(y)],
    };
    let two_step = source::Atom {
        relation: Ident::new("path", Span::call_site()),
        arguments: vec![parse_quote!(x), parse_quote!(z)],
    };
    source::Program {
        attributes: Vec::new(),
        signature: source::Signature {
            visibility: Visibility::Inherited,
            name,
            generics: Generics::default(),
        },
        relations: vec![binary_relation("edge"), binary_relation("path")],
        rules: vec![
            source::Rule {
                heads: vec![path],
                body: vec![source::BodyItem::Atom(edge.clone())],
            },
            source::Rule {
                heads: vec![two_step],
                body: vec![
                    source::BodyItem::Atom(edge),
                    source::BodyItem::Atom(source::Atom {
                        relation: Ident::new("edge", Span::call_site()),
                        arguments: vec![parse_quote!(y), parse_quote!(z)],
                    }),
                ],
            },
        ],
    }
}

fn graph_pipeline() -> CompilerPipeline {
    CompilerPipeline::new(|_, tokens| {
        let declaration = syn::parse2::<GraphDeclaration>(tokens)?;
        Ok(graph_source(declaration.name))
    })
    .unwrap()
}

#[test]
fn an_external_syntax_pack_replaces_only_source_reading() {
    let alternate = quote!(graph Program;);
    let expected = graph_pipeline()
        .expand(alternate.clone())
        .unwrap()
        .to_string();
    let mut pipeline = graph_pipeline();
    pipeline
        .compiler_mut()
        .install(&IncrementalAnalysis)
        .unwrap();

    let actual = pipeline.expand(alternate).unwrap().to_string();
    let dependencies = pipeline
        .compiler()
        .context()
        .facts()
        .relation::<IncrementalDependency>();

    assert_eq!(actual, expected);
    assert_eq!(
        dependencies,
        [
            IncrementalDependency { target: 1 },
            IncrementalDependency { target: 1 },
        ]
    );
}

#[test]
fn an_external_manual_layer_can_change_rule_planning() {
    let mut pipeline = graph_pipeline();
    pipeline.compiler_mut().install(&ManualRulePlanner).unwrap();

    let expansion = pipeline.expand(quote!(graph Program;)).unwrap().to_string();
    let reordered = pipeline
        .compiler()
        .context()
        .facts()
        .relation::<ReorderedRule>();

    assert!(expansion.contains("__miniflow_rule_0"));
    assert_eq!(reordered.len(), 1);
}

#[test]
fn an_external_incremental_layer_can_attach_facts_without_changing_rendering() {
    let baseline = graph_pipeline()
        .expand(quote!(graph Program;))
        .unwrap()
        .to_string();
    let mut pipeline = graph_pipeline();
    pipeline
        .compiler_mut()
        .install(&IncrementalAnalysis)
        .unwrap();

    let expansion = pipeline.expand(quote!(graph Program;)).unwrap().to_string();
    let dependencies = pipeline
        .compiler()
        .context()
        .facts()
        .relation::<IncrementalDependency>();

    assert_eq!(expansion, baseline);
    assert_eq!(dependencies.len(), 2);
}
