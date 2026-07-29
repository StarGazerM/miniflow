//! Procedural-macro entry point for `MiniFlow`.

use proc_macro::TokenStream;

mod driver;
mod syntax;

#[cfg(test)]
#[path = "../tests/unit/hir.rs"]
mod hir_tests;
#[cfg(test)]
#[path = "../tests/unit/pipeline.rs"]
mod pipeline_tests;

/// Compile an embedded batch-Datalog program.
#[proc_macro]
pub fn miniflow(input: TokenStream) -> TokenStream {
    driver::compile(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
