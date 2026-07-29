use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        #![profile]
        struct ReachProfile;
        .decl source(x: int32)
        .decl arc(x: int32, y: int32)
        .decl reach(x: int32)
        reach(x) :- source(x).
        reach(y) :- reach(x), arc(x, y).
    })
    .expect("canonical profiled reach expansion must compile");
    print!("{expansion}");
}
