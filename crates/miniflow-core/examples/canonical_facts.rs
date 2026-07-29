use miniflow_core::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        struct Facts;
        relation seed(i32);
        relation output(i32);
        seed(1);
        seed(2);
        output(x) <-- seed(x);
    })
    .expect("canonical facts expansion must compile");
    print!("{expansion}");
}
