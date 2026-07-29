use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Negation;
        .decl input(x: int32)
        .decl blocked(x: int32)
        .decl output(x: int32)
        output(x) :- input(x), !blocked(x).
    })
    .expect("canonical negation expansion must compile");
    print!("{expansion}");
}
