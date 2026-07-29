//! Shared compiler core for `MiniFlow` and `AscentFlow`.
//!
//! This crate is intentionally usable outside a procedural macro so expansion
//! parity tests exercise exactly the compiler invoked by the public macros.

mod canonical;
mod codegen;
pub mod compiler;
mod flowlog_fp;
pub mod hir;
mod pipeline;
pub mod plan;
pub mod rule_plan;
mod syntax;

use proc_macro2::TokenStream;
use syn::Result;

pub use canonical::extract_dataflow_core;
pub use hir::HirProgram;
pub use pipeline::{Compiler, PlanRule, RuleRequest};
pub use syntax::Program;

/// Parse and validate an embedded `MiniFlow` program.
///
/// # Errors
///
/// Returns a syntax error when the token stream is not a `MiniFlow` program.
pub fn parse(tokens: TokenStream) -> Result<Program> {
    syn::parse2(tokens)
}

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
pub fn compile(tokens: TokenStream) -> Result<TokenStream> {
    Compiler::new()?.compile(tokens)
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
pub fn compile_ascent_flow(tokens: TokenStream) -> Result<TokenStream> {
    Compiler::new()?.compile_ascent_flow(tokens)
}

/// Compile and format a complete canonical Rust expansion.
///
/// # Errors
///
/// Returns any compiler diagnostic, or a Rust parse error if the emitter
/// produces an invalid file.
pub fn compile_canonical(tokens: TokenStream) -> Result<String> {
    let emitted = compile(tokens)?;
    let file: syn::File = syn::parse2(emitted)?;
    Ok(prettyplease::unparse(&file))
}
