use std::env;

#[path = "../../../../crates/miniflow-macro/src/driver.rs"]
mod driver;
#[path = "../../../../crates/miniflow-macro/src/syntax.rs"]
mod syntax;

fn main() {
    let fixture = env::args().nth(1).expect("usage: canonical <fixture-name>");
    let tokens = miniflow_flowlog_batch_corpus::canonical_tokens(&fixture)
        .unwrap_or_else(|| panic!("fixture `{fixture}` is not strict"));
    let emitted = driver::compile(tokens)
        .unwrap_or_else(|error| panic!("canonical `{fixture}` failed: {error}"));
    let file: syn::File = syn::parse2(emitted)
        .unwrap_or_else(|error| panic!("canonical `{fixture}` emitted invalid Rust: {error}"));
    let expansion = prettyplease::unparse(&file);
    print!("{expansion}");
}
