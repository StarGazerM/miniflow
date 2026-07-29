use miniflow_core::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Negation;
        relation input(i32);
        relation blocked(i32);
        relation output(i32);
        output(x) <-- input(x), !blocked(x);
    })
    .expect("canonical negation expansion must compile");
    print!("{expansion}");
}
