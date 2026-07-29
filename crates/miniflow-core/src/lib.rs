//! Shared compiler core for `MiniFlow` and `AscentFlow`.
//!
//! This crate is intentionally usable outside a procedural macro so expansion
//! parity tests exercise exactly the compiler invoked by the public macros.

mod canonical;
mod codegen;
mod flowlog_fp;
mod hir;
pub mod syntax;

use proc_macro2::{Ident, Span, TokenStream};
use syn::Result;

pub use canonical::extract_dataflow_core;
pub use hir::HirProgram;
pub use syntax::Program;

/// Lower a parsed program into relation-identified HIR and dependency SCCs.
///
/// # Errors
///
/// Returns a semantic error for invalid attributes, relation references, or
/// arities.
pub fn lower(program: Program) -> Result<HirProgram> {
    HirProgram::lower(program)
}

/// Execute the implemented compiler stages.
///
/// Code emission is introduced after the relational HIR and SCC invariants are
/// pinned. Keeping this entry point now ensures the proc macro and tests cannot
/// grow separate stage drivers.
///
/// # Errors
///
/// Returns any syntax or semantic error reported by the shared compiler
/// stages.
pub fn compile(program: Program) -> Result<TokenStream> {
    let hir = lower(program)?;
    hir.emit()
}

/// Compile an embedded program against the `ascent-flow` runtime façade.
///
/// The syntax, HIR, SCC schedule, and dataflow emitter are shared with
/// [`compile`]; only the absolute public-crate path in generated Rust differs.
///
/// # Errors
///
/// Returns any syntax or semantic error reported by the shared compiler
/// stages.
pub fn compile_ascent_flow(program: Program) -> Result<TokenStream> {
    let mut hir = lower(program)?;
    hir.runtime_crate = Ident::new("ascent_flow", Span::call_site());
    hir.emit()
}

/// Compile and format a complete canonical Rust expansion.
///
/// # Errors
///
/// Returns any compiler diagnostic, or a Rust parse error if the emitter
/// produces an invalid file.
pub fn compile_canonical(program: Program) -> Result<String> {
    let emitted = compile(program)?;
    let file: syn::File = syn::parse2(emitted)?;
    Ok(prettyplease::unparse(&file))
}
