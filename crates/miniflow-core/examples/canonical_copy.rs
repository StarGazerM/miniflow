use miniflow_core::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Copy;
        relation input(i32, i32);
        relation output(i32, i32);
        output(x, y) <-- input(x, y);
    })
    .expect("canonical copy expansion must compile");
    print!("{expansion}");
}
