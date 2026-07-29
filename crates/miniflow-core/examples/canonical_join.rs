use miniflow_core::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Join;
        relation left(i32, i32);
        relation right(i32, i32);
        relation output(i32, i32);
        output(x, z) <-- left(x, y), right(y, z);
    })
    .expect("canonical join expansion must compile");
    print!("{expansion}");
}
