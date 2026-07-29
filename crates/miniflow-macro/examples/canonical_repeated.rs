mod common;

use common::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Repeated;
        relation input(i32, i32);
        relation output(i32);
        output(x) :- input(x, x);
    })
    .expect("canonical repeated-variable expansion must compile");
    print!("{expansion}");
}
