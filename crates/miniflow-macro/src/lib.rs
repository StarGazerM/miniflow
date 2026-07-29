//! Procedural-macro entry point for FlowLog-syntax `MiniFlow`.

use proc_macro::TokenStream;

/// Compile an embedded batch-Datalog program.
#[proc_macro]
pub fn miniflow(input: TokenStream) -> TokenStream {
    miniflow_syntax::parse(input.into())
        .and_then(miniflow_core::compile)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
