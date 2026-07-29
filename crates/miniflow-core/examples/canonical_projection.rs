use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Projection;
        .decl input(x: int32, y: int32)
        .decl output(x: int32)
        output(x) :- input(x, _).
    })
    .expect("canonical projection expansion must compile");
    print!("{expansion}");
}
