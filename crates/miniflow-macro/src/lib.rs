//! Procedural-macro entry point for `MiniFlow`.

use proc_macro::TokenStream;

/// Compile an embedded batch-Datalog program.
#[proc_macro]
pub fn miniflow(input: TokenStream) -> TokenStream {
    miniflow_core::compile(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
