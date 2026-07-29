use miniflow_syntax::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Join;
        .decl left(x: int32, y: int32)
        .decl right(y: int32, z: int32)
        .decl output(x: int32, z: int32)
        output(x, z) :- left(x, y), right(y, z).
    })
    .expect("canonical join expansion must compile");
    print!("{expansion}");
}
