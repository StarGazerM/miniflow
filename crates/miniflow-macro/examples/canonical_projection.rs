mod common;

use common::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Projection;
        relation input(i32, i32);
        relation output(i32);
        output(x) :- input(x, _);
    })
    .expect("canonical projection expansion must compile");
    print!("{expansion}");
}
