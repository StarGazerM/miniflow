use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Union;
        .decl left(x: int32)
        .decl right(x: int32)
        .decl output(x: int32)
        output(x) :- left(x).
        output(x) :- right(x).
    })
    .expect("canonical union expansion must compile");
    print!("{expansion}");
}
