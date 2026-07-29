use miniflow_core::compile_canonical;
use quote::quote;

fn main() {
    let expansion = compile_canonical(quote! {
        #![profile]
        struct ReachProfile;
        relation source(i32);
        relation arc(i32, i32);
        relation reach(i32);
        reach(x) <-- source(x);
        reach(y) <-- reach(x), arc(x, y);
    })
    .expect("canonical profiled reach expansion must compile");
    print!("{expansion}");
}
