use std::env;

fn main() {
    let fixture = env::args().nth(1).expect("usage: canonical <fixture-name>");
    let tokens = miniflow_flowlog_batch_corpus::canonical_tokens(&fixture)
        .unwrap_or_else(|| panic!("fixture `{fixture}` is not strict"));
    let expansion = miniflow_syntax::compile_canonical(tokens)
        .unwrap_or_else(|error| panic!("canonical `{fixture}` failed: {error}"));
    print!("{expansion}");
}
