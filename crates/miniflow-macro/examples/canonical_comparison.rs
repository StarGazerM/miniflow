mod common;

use common::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Comparison;
        relation input(i32, i32);
        relation output(i32, i32);
        output(x, y) :- input(x, y), if x < y;
    })
    .expect("canonical comparison expansion must compile");
    print!("{expansion}");
}
