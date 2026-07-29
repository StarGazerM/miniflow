use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Facts;
        .decl seed(x: int32)
        .decl output(x: int32)
        seed(1).
        seed(2).
        output(x) :- seed(x).
    })
    .expect("canonical facts expansion must compile");
    print!("{expansion}");
}
