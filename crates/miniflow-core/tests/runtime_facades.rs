use miniflow_core::{compile, compile_ascent_flow};
use quote::quote;

#[test]
fn public_facades_differ_only_by_the_runtime_crate_path() {
    let flowlog_source = quote! {
        #![profile]

        struct Mean;

        .decl value(value: int32)
        .decl mean(value: int32)

        mean(mean(value)) :- value(value).
    };
    let ascent_source = quote! {
        #![profile]

        struct Mean;

        relation value(i32);
        relation mean(i32);

        mean(result.round() as i32) <-- agg result = mean(value) in value(value);
    };

    let miniflow = compile(miniflow_syntax::parse(flowlog_source).unwrap())
        .unwrap()
        .to_string();
    let ascent_flow = compile_ascent_flow(ascent_flow_syntax::parse(ascent_source).unwrap())
        .unwrap()
        .to_string();

    assert!(!miniflow.contains("ascent_flow"));
    assert!(ascent_flow.contains("ascent_flow"));
    assert_eq!(miniflow, ascent_flow.replace("ascent_flow", "miniflow"));
}
