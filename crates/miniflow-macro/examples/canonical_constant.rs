mod common;

use common::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Constant;
        relation input(i32, i32);
        relation output(i32);
        output(x) :- input(x, 10);
    })
    .expect("canonical constant-filter expansion must compile");
    print!("{expansion}");
}
