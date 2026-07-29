//! Procedural-macro entry points for `MiniFlow` and `AscentFlow`.

use proc_macro::TokenStream;

/// Compile an embedded batch-Datalog program.
#[proc_macro]
pub fn miniflow(input: TokenStream) -> TokenStream {
    miniflow_core::compile(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Compile an Ascent-shaped embedded batch-Datalog program.
#[proc_macro]
pub fn ascent_flow(input: TokenStream) -> TokenStream {
    miniflow_core::compile_ascent_flow(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
