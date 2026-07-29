mod common;

use common::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Union;
        relation left(i32);
        relation right(i32);
        relation output(i32);
        output(x) :- left(x);
        output(x) :- right(x);
    })
    .expect("canonical union expansion must compile");
    print!("{expansion}");
}
