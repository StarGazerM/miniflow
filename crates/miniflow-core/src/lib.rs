//! Extensible compiler core and default frontend for `MiniFlow`.
//!
//! This crate is intentionally usable outside a procedural macro so expansion
//! parity tests exercise exactly the compiler invoked by the public macros.

mod canonical;
pub mod compiler;
mod dd_render;
mod flowlog_analysis;
mod flowlog_fp;
pub mod flowlog_plan;
pub mod hir;
mod pipeline;
pub mod plan;
pub mod program_plan;
pub mod rule_plan;
pub mod scc_plan;
pub mod source;
mod syntax;

use proc_macro2::TokenStream;
use syn::Result;

pub use canonical::extract_dataflow_core;
pub use hir::HirProgram;
pub use pipeline::{
    Compiler, CompilerPipeline, CompilerStage, PlanRule, PlanScc, PlanningCatalog, RuleRequest,
    SccRequest,
};
pub use source::Program;
pub use source::Program as SourceProgram;

#[cfg(test)]
#[path = "../tests/unit/default_hir.rs"]
mod default_hir_tests;
#[cfg(test)]
#[path = "../tests/unit/default_pipeline.rs"]
mod default_pipeline_tests;

/// Construct the default `MiniFlow` surface-to-Rust pipeline.
///
/// # Errors
///
/// Returns a diagnostic if the standard planner cannot be installed.
pub fn default_pipeline() -> Result<CompilerPipeline> {
    CompilerPipeline::new(|_, tokens| syntax::parse(tokens))
}

/// Expand one program with the default pipeline.
///
/// # Errors
///
/// Returns syntax, semantic, planning, or rendering diagnostics.
pub fn compile(tokens: TokenStream) -> Result<TokenStream> {
    default_pipeline()?.expand(tokens)
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
