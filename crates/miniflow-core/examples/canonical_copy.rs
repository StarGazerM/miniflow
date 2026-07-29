use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Copy;
        .decl input(x: int32, y: int32)
        .decl output(x: int32, y: int32)
        output(x, y) :- input(x, y).
    })
    .expect("canonical copy expansion must compile");
    print!("{expansion}");
}
