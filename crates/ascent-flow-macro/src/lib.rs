//! Procedural-macro entry point for Ascent-shaped `AscentFlow`.

use proc_macro::TokenStream;

/// Compile an Ascent-shaped embedded batch-Datalog program.
#[proc_macro]
pub fn ascent_flow(input: TokenStream) -> TokenStream {
    ascent_flow_syntax::parse(input.into())
        .and_then(miniflow_core::compile_ascent_flow)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
