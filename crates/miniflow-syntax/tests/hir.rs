use quote::quote;

use miniflow_core::lower;
use miniflow_syntax::parse;

#[test]
fn infers_recursive_and_non_recursive_sccs() {
    let program = parse(quote! {
        pub struct Reach;
        .decl source(x: int32)
        .decl arc(x: int32, y: int32)
        .decl reach(x: int32)
        reach(x) :- source(x).
        reach(y) :- reach(x), arc(x, y).
    })
    .unwrap();
    let hir = lower(program).unwrap();

    assert_eq!(hir.sccs.len(), 2);
    assert!(!hir.sccs[0].recursive);
    assert!(hir.sccs[1].recursive);
}

#[test]
fn rejects_undeclared_relations() {
    let program = parse(quote! {
        struct Bad;
        .decl output(x: int32)
        output(x) :- missing(x).
    })
    .unwrap();
    let error = lower(program).unwrap_err();
    assert!(error.to_string().contains("`missing` is not declared"));
}

#[test]
fn emits_rust_typed_relation_fields_without_a_miniflow_type_system() {
    let program = parse(quote! {
        pub struct P;
        .decl edge(name: ::std::sync::Arc<str>, index: usize)
    })
    .unwrap();
    let hir = lower(program).unwrap();
    let file: syn::File = syn::parse2(hir.emit_declarations()).unwrap();
    let rendered = prettyplease::unparse(&file);

    assert!(rendered.contains("pub edge: ::std::vec::Vec<(::std::sync::Arc<str>, usize)>"));
}

#[test]
fn rejects_negation_inside_a_recursive_scc() {
    let program = parse(quote! {
        struct Bad;
        .decl p(x: int32)
        .decl q(x: int32)
        p(x) :- q(x).
        q(x) :- !p(x), q(x).
    })
    .unwrap();
    let error = lower(program).unwrap_err();
    assert!(error.to_string().contains("negation is not stratified"));
}
